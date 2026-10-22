//! The scripted discovery source `Script` and its replies `ScriptReply` (part of `test_support.rs`).

use super::*;

/// Reply of `publish` of the `Script` source.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ScriptReply {
    Ok,
    Refused,
    Timeout,
    Forever,
    Panic,
}

/// A scripted discovery source: returns the given list and waits forever; the `publish` reply is set.
pub(crate) struct Script {
    name: &'static str,
    publishes: bool,
    reply: ScriptReply,
    reply_after: Duration,
    finds: Vec<PeerAddr>,
    ends: bool,
    journal: Option<Journal>,
    runs: AtomicUsize,
    publish_calls: AtomicUsize,
    published: Mutex<Vec<PeerAddr>>,
    dropped: Arc<AtomicBool>,
}

impl Script {
    /// A reading source, finds nothing.
    pub(crate) fn new(name: &'static str) -> Self {
        Script {
            name,
            publishes: false,
            reply: ScriptReply::Ok,
            reply_after: Duration::ZERO,
            finds: Vec::new(),
            ends: false,
            journal: None,
            runs: AtomicUsize::new(0),
            publish_calls: AtomicUsize::new(0),
            published: Mutex::new(Vec::new()),
            dropped: Arc::new(AtomicBool::new(false)),
        }
    }

    /// A publishing source with this `publish` reply.
    pub(crate) fn publisher(mut self, reply: ScriptReply) -> Self {
        (self.publishes, self.reply) = (true, reply);
        self
    }

    /// `publish` replies not at once but after `after`.
    pub(crate) fn replying_after(mut self, after: Duration) -> Self {
        self.reply_after = after;
        self
    }

    /// `run_source` returns these addresses right after the start.
    pub(crate) fn finding(mut self, addrs: Vec<PeerAddr>) -> Self {
        self.finds = addrs;
        self
    }

    /// `run_source` ends by itself after returning the list.
    pub(crate) fn ending(mut self) -> Self {
        self.ends = true;
        self
    }

    /// Dropping its own `run_source` writes "discovery" to the journal.
    pub(crate) fn journaled(mut self, journal: &Journal) -> Self {
        self.journal = Some(journal.clone());
        self
    }

    pub(crate) fn shared(self) -> Arc<Script> {
        Arc::new(self)
    }

    /// How many times `run_source` was called.
    pub(crate) fn runs(&self) -> usize {
        self.runs.load(Ordering::SeqCst)
    }

    /// How many times `publish` was called.
    pub(crate) fn publish_calls(&self) -> usize {
        self.publish_calls.load(Ordering::SeqCst)
    }

    /// Addresses `publish` was called with, in order.
    pub(crate) fn published(&self) -> Vec<PeerAddr> {
        lock(&self.published).clone()
    }

    /// Whether the future of `run_source` was dropped (cancelled or finished).
    pub(crate) fn dropped(&self) -> bool {
        self.dropped.load(Ordering::SeqCst)
    }
}

async fn answer(reply: ScriptReply, after: Duration) -> NetResult<()> {
    tokio::time::sleep(after).await;
    match reply {
        ScriptReply::Ok => Ok(()),
        ScriptReply::Refused => Err(NetError::Refused),
        ScriptReply::Timeout => Err(NetError::Timeout),
        ScriptReply::Forever => std::future::pending().await,
        ScriptReply::Panic => panic!("publish of Script panicked on purpose"),
    }
}

impl Discovery for Script {
    fn name(&self) -> &'static str {
        self.name
    }

    fn publishes(&self) -> bool {
        self.publishes
    }

    fn publish(&self, own_addr: &PeerAddr) -> BoxFuture<'_, NetResult<()>> {
        self.publish_calls.fetch_add(1, Ordering::SeqCst);
        lock(&self.published).push(own_addr.clone());
        Box::pin(answer(self.reply, self.reply_after))
    }

    fn run_source(self: Arc<Self>, out: mpsc::Sender<PeerAddr>) -> BoxFuture<'static, ()> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        let flag = Some(self.dropped.clone());
        let mark = Mark {
            journal: self.journal.clone(),
            what: "discovery",
            flag,
        };
        Box::pin(async move {
            let _mark = mark;
            for found in &self.finds {
                if out.send(found.clone()).await.is_err() {
                    return;
                }
            }
            if !self.ends {
                std::future::pending::<()>().await;
            }
        })
    }
}
