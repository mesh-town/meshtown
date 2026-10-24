//! Tests of the saved DHT table file format and its load / save (`file.rs`).

use super::*;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::{Path, PathBuf};

const NOW: u64 = 1_000_000;

// ---------- reading text (`parse`) ----------

#[test]
fn parse_reads_one_node() {
    let text = text_with(NOW, &["1.2.3.4:5"]);

    let nodes = parse(&text, NOW, TTL).expect("a valid file");

    assert_eq!(nodes, vec![addr("1.2.3.4:5")]);
}

#[test]
fn parse_keeps_exactly_256_nodes() {
    let all = nodes(MAX_NODES);
    let text = render_lines(NOW, &all);

    let got = parse(&text, NOW, TTL).expect("a valid file");

    assert_eq!(got, all);
}

#[test]
fn parse_caps_at_256_nodes_freshest_first() {
    let all = nodes(MAX_NODES + 44);
    let text = render_lines(NOW, &all);

    let got = parse(&text, NOW, TTL).expect("a valid file");

    assert_eq!(got.len(), MAX_NODES, "the list must be capped");
    assert_eq!(got, all[..MAX_NODES], "the first lines are the freshest");
}

#[test]
fn parse_without_nodes_is_rejected() {
    let text = format!("townjump-nodes 1\nsaved {NOW}\n");

    assert_eq!(parse(&text, NOW, TTL), Err("no nodes"));
}

#[test]
fn parse_rejects_missing_or_foreign_header() {
    let foreign = format!("other-format 1\nsaved {NOW}\n1.2.3.4:5\n");
    let headless = format!("saved {NOW}\n1.2.3.4:5\n");

    assert_eq!(parse("", NOW, TTL), Err("no header"), "empty text");
    assert_eq!(parse(&headless, NOW, TTL), Err("no header"), "no header");
    assert_eq!(
        parse(&foreign, NOW, TTL),
        Err("no header"),
        "foreign header"
    );
}

#[test]
fn parse_rejects_bad_stamp() {
    let missing = "townjump-nodes 1\n1.2.3.4:5\n";
    let not_a_number = "townjump-nodes 1\nsaved yesterday\n1.2.3.4:5\n";
    let header_only = "townjump-nodes 1\n";

    assert_eq!(parse(missing, NOW, TTL), Err("no stamp"), "no stamp line");
    assert_eq!(
        parse(not_a_number, NOW, TTL),
        Err("no stamp"),
        "stamp is not a number"
    );
    assert_eq!(
        parse(header_only, NOW, TTL),
        Err("no stamp"),
        "nothing after the header"
    );
}

#[test]
fn parse_accepts_age_equal_to_ttl() {
    let text = text_with(NOW - TTL.as_secs(), &["1.2.3.4:5"]);

    assert_eq!(parse(&text, NOW, TTL), Ok(vec![addr("1.2.3.4:5")]));
}

#[test]
fn parse_rejects_age_over_ttl() {
    let text = text_with(NOW - TTL.as_secs() - 1, &["1.2.3.4:5"]);

    assert_eq!(parse(&text, NOW, TTL), Err("too old"));
}

#[test]
fn parse_accepts_stamp_from_the_future() {
    let text = text_with(NOW + 86_400, &["1.2.3.4:5"]);

    assert_eq!(
        parse(&text, NOW, TTL),
        Ok(vec![addr("1.2.3.4:5")]),
        "a clock that was behind when the file was written must not make it stale"
    );
}

#[test]
fn parse_skips_bad_lines() {
    let text = text_with(
        NOW,
        &[
            "1.2.3.4:5",
            "garbage",
            "1.2.3.4:0",
            "[::1]:80",
            "1.2.3.4",
            "999.1.1.1:5",
            "",
            "5.6.7.8:9",
        ],
    );

    let got = parse(&text, NOW, TTL).expect("a valid file");

    assert_eq!(
        got,
        vec![addr("1.2.3.4:5"), addr("5.6.7.8:9")],
        "only well-formed IPv4 lines with a non-zero port stay"
    );
}

#[test]
fn parse_drops_duplicates_keeping_order() {
    let text = text_with(
        NOW,
        &[
            "5.6.7.8:9",
            "1.2.3.4:5",
            "5.6.7.8:9",
            "1.2.3.4:5",
            "9.9.9.9:1",
        ],
    );

    let got = parse(&text, NOW, TTL).expect("a valid file");

    assert_eq!(
        got,
        vec![addr("5.6.7.8:9"), addr("1.2.3.4:5"), addr("9.9.9.9:1")]
    );
}

#[test]
fn parse_accepts_crlf_lines() {
    let text = format!("townjump-nodes 1\r\nsaved {NOW}\r\n1.2.3.4:5\r\n5.6.7.8:9\r\n");

    let got = parse(&text, NOW, TTL).expect("CRLF is a valid line ending");

    assert_eq!(got, vec![addr("1.2.3.4:5"), addr("5.6.7.8:9")]);
}

// ---------- writing text (`render`) ----------

#[test]
fn render_then_parse_round_trips() {
    let all = vec![
        addr("1.2.3.4:5"),
        addr("6.7.8.9:10"),
        addr("10.0.0.1:65535"),
    ];

    let text = render(&all, NOW);

    assert!(
        text.starts_with(&format!("townjump-nodes 1\nsaved {NOW}\n1.2.3.4:5\n")),
        "the layout is shared with the townjump CLI: header, stamp, one node per line: {text:?}"
    );
    assert_eq!(parse(&text, NOW, TTL), Ok(all));
}

#[test]
fn render_writes_at_most_256_nodes() {
    let all = nodes(300);

    let text = render(&all, NOW);

    assert_eq!(
        text.lines().count(),
        2 + MAX_NODES,
        "header, stamp and the capped list"
    );
    assert_eq!(parse(&text, NOW, TTL), Ok(all[..MAX_NODES].to_vec()));
}

// ---------- the file (`load`, `save`) ----------

#[test]
fn load_missing_file_gives_empty() {
    let dir = Scratch::new("missing");
    let path = dir.file("dht-nodes.txt");
    let before = journal_mark();

    let got = load(&path, NOW, TTL);

    assert!(got.is_empty(), "no file is a normal first start");
    assert!(
        journal_about(&path, before).is_empty(),
        "a missing file must not be logged"
    );
}

#[test]
fn load_corrupt_file_gives_empty() {
    let dir = Scratch::new("corrupt");
    let path = dir.file("dht-nodes.txt");
    std::fs::write(&path, "this is not a table\n").expect("write the file");
    let before = journal_mark();

    let got = load(&path, NOW, TTL);

    assert!(got.is_empty());
    let lines = journal_about(&path, before);
    assert_eq!(
        lines.len(),
        1,
        "exactly one journal line expected: {lines:?}"
    );
    assert!(
        lines[0].contains("not used"),
        "the line says why: {lines:?}"
    );
}

#[test]
fn load_binary_file_gives_empty() {
    let dir = Scratch::new("binary");
    let path = dir.file("dht-nodes.txt");
    std::fs::write(&path, [0xffu8, 0xfe, 0x00, 0x9f, 0x92]).expect("write the file");
    let before = journal_mark();

    let got = load(&path, NOW, TTL);

    assert!(got.is_empty());
    assert_eq!(
        journal_about(&path, before).len(),
        1,
        "a file that is not UTF-8 is reported once"
    );
}

#[test]
fn save_replaces_file_and_leaves_no_temp() {
    let dir = Scratch::new("replace");
    let path = dir.file("dht-nodes.txt");
    std::fs::write(&path, "old contents\n").expect("write the old file");
    let all = vec![addr("1.2.3.4:5"), addr("6.7.8.9:10")];

    save(&path, &all, NOW).expect("save");

    let text = std::fs::read_to_string(&path).expect("read back");
    assert_eq!(text, render(&all, NOW));
    assert_eq!(
        dir.names(),
        vec!["dht-nodes.txt".to_string()],
        "no temporary file may stay beside the table"
    );
}

#[test]
fn save_into_missing_dir_fails() {
    let dir = Scratch::new("no-dir");
    let path = dir.file("absent").join("dht-nodes.txt");

    let res = save(&path, &[addr("1.2.3.4:5")], NOW);

    assert!(res.is_err(), "the directory is not created");
    assert!(!dir.file("absent").exists(), "no directory may appear");
    assert!(dir.names().is_empty(), "nothing may be left in the parent");
}

// ---------- helpers ----------

fn addr(s: &str) -> SocketAddrV4 {
    s.parse().expect("test address")
}

/// `count` distinct addresses in a fixed order.
fn nodes(count: usize) -> Vec<SocketAddrV4> {
    (0..count)
        .map(|i| {
            let ip = Ipv4Addr::new(10, 0, (i / 256) as u8, (i % 256) as u8);
            SocketAddrV4::new(ip, 4000)
        })
        .collect()
}

/// Valid header, the stamp, then the lines as they are.
fn text_with(saved: u64, lines: &[&str]) -> String {
    let mut text = format!("townjump-nodes 1\nsaved {saved}\n");
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// A file body written by hand: does not go through `render`, so no cap is applied.
fn render_lines(saved: u64, all: &[SocketAddrV4]) -> String {
    let lines: Vec<String> = all.iter().map(|a| a.to_string()).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    text_with(saved, &refs)
}

/// The last journal sequence number before the code under test runs.
fn journal_mark() -> u64 {
    crate::node::output::log_page(0, 1).last_seq
}

/// Texts of the `hyper` journal lines written after `mark` that name `path`.
fn journal_about(path: &Path, mark: u64) -> Vec<String> {
    let shown = path.display().to_string();
    crate::node::output::log_page(mark, 100_000)
        .lines
        .into_iter()
        .filter(|l| l.tag == "hyper" && l.text.contains(&shown))
        .map(|l| l.text)
        .collect()
}

/// A temporary directory removed on drop.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "mesh-dht-table-{name}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("could not create the test's temporary directory");
        Self { dir }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Sorted names of the entries directly in the directory.
    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.dir)
            .expect("read the directory")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
