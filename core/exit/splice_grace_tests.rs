//! Tests of the wait for the second half in `splice` / `splice_windowed`: it is measured from idleness,
//! not from the moment the first half closed. Time is paused; the wires are those of `splice_tests.rs`.

use super::*;

/// One notice per whole chunk the client has read.
type Arrivals = tokio::sync::mpsc::UnboundedReceiver<()>;
/// The reader task: bytes read in total, and the splice task handed back.
type Reader = JoinHandle<(usize, JoinHandle<(u64, u64)>)>;

const CHUNK: &[u8] = &[9u8; 1000];
const CHUNK_EVERY: Duration = Duration::from_secs(20);
/// Longer than three full graces, so an absolute wait would cut the download.
const DOWNLOAD_FOR: Duration = Duration::from_secs(200);
/// Generous test-side limit: virtual time, so it costs nothing.
const OUTER: Duration = Duration::from_secs(1000);

/// Lets a loopback write reach the reader's socket in real time: virtual time must not jump
/// over bytes the kernel has not yet delivered.
fn settle() {
    std::thread::sleep(Duration::from_millis(20));
}

/// Lets the other tasks run and the kernel deliver, without the runtime ever going idle.
async fn pump() {
    settle();
    tokio::task::yield_now().await;
}

/// Waits for the reader's notice without ever idling the runtime (an idle runtime lets the paused clock jump).
async fn await_chunk(arrived: &mut Arrivals) {
    for _ in 0..500 {
        pump().await;
        if arrived.try_recv().is_ok() {
            return;
        }
    }
    panic!("the chunk never arrived");
}

/// Reads the client side chunk by chunk, telling the writer each time a whole chunk has arrived:
/// with paused time the writer must not let the clock run ahead of the bytes still in flight.
fn read_chunks(client_in: DuplexStream, task: JoinHandle<(u64, u64)>) -> (Arrivals, Reader) {
    let mut client_in = client_in;
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let reader = crate::node::output::spawn(async move {
        let mut total = 0;
        let mut chunk = vec![0u8; CHUNK.len()];
        while client_in.read_exact(&mut chunk).await.is_ok() {
            total += chunk.len();
            let _ = tx.send(());
        }
        (total, task)
    });
    (rx, reader)
}

async fn download_outliving_the_grace(window: Option<usize>) {
    // given: the client sent a request and closed its side; the site answers for 200 s in chunks
    let mut w = setup_with(window).await;
    w.client_out.write_all(b"GET").await.expect("client write");
    w.client_out.shutdown().await.expect("client close");
    // No waiting on the socket here: an idle runtime lets the paused clock jump ahead of the bytes.
    let mut request = [0u8; 3];
    let got = loop {
        pump().await;
        if let Ok(n) = w.site.try_read(&mut request) {
            break n;
        }
    };
    assert_eq!(got, 3, "the request reached the site");
    let chunks = (DOWNLOAD_FOR.as_secs() / CHUNK_EVERY.as_secs()) as usize;
    assert!(DOWNLOAD_FOR >= HALF_CLOSE_GRACE * 3);

    // when: the site keeps sending, the client reads everything, then the site closes
    let (mut arrived, reader) = read_chunks(w.client_in, w.task);
    for i in 0..chunks {
        tokio::time::sleep(CHUNK_EVERY).await;
        w.site
            .write_all(CHUNK)
            .await
            .unwrap_or_else(|e| panic!("site write {i}: {e}"));
        settle();
        await_chunk(&mut arrived).await;
    }
    w.site.shutdown().await.expect("site close");
    while !reader.is_finished() {
        pump().await;
    }

    // then: every chunk arrived, splice ended with (request, everything)
    let (got, task) = tokio::time::timeout(OUTER, reader)
        .await
        .expect("client read finished")
        .expect("reader didn't panic");
    assert_eq!(got, chunks * CHUNK.len(), "the download was cut short");
    let counts = tokio::time::timeout(OUTER, task)
        .await
        .expect("splice ended");
    assert_eq!(
        counts.expect("no panic"),
        (3, (chunks * CHUNK.len()) as u64)
    );
}

#[tokio::test(start_paused = true)]
async fn download_outliving_the_grace_is_not_cut() {
    download_outliving_the_grace(None).await;
}

#[tokio::test(start_paused = true)]
async fn download_outliving_the_grace_is_not_cut_with_a_window() {
    download_outliving_the_grace(Some(16 * 1024)).await;
}

#[tokio::test(start_paused = true)]
async fn second_half_idle_for_the_grace_is_cut() {
    // given: the client sent bytes and closed; the site says nothing
    let mut w = setup().await;
    w.client_out
        .write_all(b"hello")
        .await
        .expect("client write");
    w.client_out.shutdown().await.expect("client close");

    // when: nearly one grace passes
    let early = HALF_CLOSE_GRACE - Duration::from_secs(1);
    let still = tokio::time::timeout(early, &mut w.task).await;

    // then: splice is still waiting, and ends within two graces with nothing from the site
    assert!(still.is_err(), "cut before the grace expired");
    let late = HALF_CLOSE_GRACE * 2 + Duration::from_secs(1);
    let ended = tokio::time::timeout(late, &mut w.task).await;
    let counts = ended.expect("the idle half must be cut");
    assert_eq!(counts.expect("no panic"), (5, 0));
}

#[tokio::test(start_paused = true)]
async fn trickle_after_a_silent_first_side_is_not_cut() {
    // given: the client closed without a byte; the site sends a chunk every 3 s for 30 s
    let mut w = setup().await;
    w.client_out.shutdown().await.expect("client close");
    let (mut arrived, reader) = read_chunks(w.client_in, w.task);

    // when: the trickle goes on past several silent graces
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_secs(3)).await;
        w.site.write_all(CHUNK).await.expect("site write");
        settle();
        await_chunk(&mut arrived).await;
    }
    w.site.shutdown().await.expect("site close");
    while !reader.is_finished() {
        pump().await;
    }

    // then: all chunks arrived
    let (got, task) = tokio::time::timeout(OUTER, reader)
        .await
        .expect("client read finished")
        .expect("reader didn't panic");
    assert_eq!(got, 10 * CHUNK.len(), "the trickle was cut");
    let counts = tokio::time::timeout(OUTER, task)
        .await
        .expect("splice ended");
    assert_eq!(counts.expect("no panic"), (0, 10_000));
}
