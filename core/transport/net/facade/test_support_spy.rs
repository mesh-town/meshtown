//! Spy transport for the facade tests (part of `test_support.rs`).

use super::*;

/// A spy transport on top of another transport: passes everything through and remembers the calls.
pub(crate) struct Spy {
    inner: Arc<dyn Transport>,
    kind: Option<&'static str>,
    serve_all: bool,
    journal: Option<Journal>,
    closes: AtomicUsize,
    connects: Mutex<Vec<(PeerId, ServiceId)>>,
    hints: Mutex<Vec<PeerAddr>>,
    relays: Mutex<Vec<Vec<RelayRef>>>,
    served: Mutex<Vec<Vec<ServiceId>>>,
    proven: Mutex<Vec<(PeerId, std::net::SocketAddr)>>,
    /// Incoming connections of the `serving_everything` mode: while the spy holds them, only the facade can close them.
    held: Mutex<Vec<Arc<dyn PeerConn>>>,
}

impl Spy {
    pub(crate) fn over(inner: Arc<dyn Transport>) -> Self {
        Spy {
            inner,
            kind: None,
            serve_all: false,
            journal: None,
            closes: AtomicUsize::new(0),
            connects: Mutex::new(Vec::new()),
            hints: Mutex::new(Vec::new()),
            relays: Mutex::new(Vec::new()),
            served: Mutex::new(Vec::new()),
            proven: Mutex::new(Vec::new()),
            held: Mutex::new(Vec::new()),
        }
    }

    /// The `mem` transport of node `secret` in network `net`.
    pub(crate) fn on(net: &Arc<MemNet>, secret: &NodeSecret) -> Self {
        Self::over(net.transport(secret))
    }

    /// Its own `kind()` instead of `mem`.
    pub(crate) fn of_kind(mut self, kind: &'static str) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Violates the `serve` contract: hands the lower transport all three services of the project.
    pub(crate) fn serving_everything(mut self) -> Self {
        self.serve_all = true;
        self
    }

    /// `close` writes "transport" to the journal.
    pub(crate) fn journaled(mut self, journal: &Journal) -> Self {
        self.journal = Some(journal.clone());
        self
    }

    /// `proven_addr` answers `at` for `peer`.
    pub(crate) fn proving(self, peer: PeerId, at: std::net::SocketAddr) -> Self {
        lock(&self.proven).push((peer, at));
        self
    }

    pub(crate) fn shared(self) -> Arc<Spy> {
        Arc::new(self)
    }

    /// How many times `close` was called.
    pub(crate) fn closes(&self) -> usize {
        self.closes.load(Ordering::SeqCst)
    }

    /// Calls of `connect`: to whom and for which service.
    pub(crate) fn connects(&self) -> Vec<(PeerId, ServiceId)> {
        lock(&self.connects).clone()
    }

    /// Addresses passed to `add_hints`.
    pub(crate) fn hints(&self) -> Vec<PeerAddr> {
        lock(&self.hints).clone()
    }

    /// Lists passed to `set_relays`.
    pub(crate) fn relays(&self) -> Vec<Vec<RelayRef>> {
        lock(&self.relays).clone()
    }

    /// Lists passed to `serve`.
    pub(crate) fn served(&self) -> Vec<Vec<ServiceId>> {
        lock(&self.served).clone()
    }

    async fn close_inner(&self) {
        self.closes.fetch_add(1, Ordering::SeqCst);
        if let Some(journal) = &self.journal {
            journal.push("transport");
        }
        self.inner.close().await;
    }
}

impl Transport for Spy {
    fn kind(&self) -> &'static str {
        self.kind.unwrap_or_else(|| self.inner.kind())
    }

    fn me(&self) -> PeerId {
        self.inner.me()
    }

    fn local_addr(&self) -> PeerAddr {
        self.inner.local_addr()
    }

    fn can_reach(&self, to: &PeerAddr) -> bool {
        self.inner.can_reach(to)
    }

    fn proven_addr(&self, peer: PeerId) -> Option<std::net::SocketAddr> {
        let own = lock(&self.proven)
            .iter()
            .find(|(id, _)| *id == peer)
            .map(|(_, at)| *at);
        own.or_else(|| self.inner.proven_addr(peer))
    }

    fn add_hints(&self, addr: &PeerAddr) {
        lock(&self.hints).push(addr.clone());
        self.inner.add_hints(addr);
    }

    fn set_relays(&self, relays: &[RelayRef]) {
        lock(&self.relays).push(relays.to_vec());
        self.inner.set_relays(relays);
    }

    fn serve(&self, svcs: &[ServiceId]) {
        lock(&self.served).push(svcs.to_vec());
        match self.serve_all {
            true => self.inner.serve(&[AUTH, EXIT, PORT]),
            false => self.inner.serve(svcs),
        }
    }

    fn connect(
        &self,
        to: &PeerAddr,
        svc: ServiceId,
    ) -> BoxFuture<'_, NetResult<Arc<dyn PeerConn>>> {
        lock(&self.connects).push((to.id, svc));
        self.inner.connect(to, svc)
    }

    fn accept(&self) -> BoxFuture<'_, Option<(ServiceId, Arc<dyn PeerConn>)>> {
        Box::pin(async move {
            let next = self.inner.accept().await;
            if let (true, Some((_, conn))) = (self.serve_all, &next) {
                lock(&self.held).push(conn.clone());
            }
            next
        })
    }

    fn net_info(&self) -> NetInfo {
        self.inner.net_info()
    }

    fn wait_net_info(&self) -> BoxFuture<'_, ()> {
        self.inner.wait_net_info()
    }

    fn close(&self) -> BoxFuture<'_, ()> {
        Box::pin(self.close_inner())
    }
}
