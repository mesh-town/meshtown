//! The process-wide proxy listener: binds the port, accepts clients and serves them through the current target.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;

use crate::config::config_file::{IfaceFilter, ListenAccess, ProcessSettings};
use crate::node::output::say;
use crate::proxy::route::RouteCtx;
use crate::proxy::serve::serve_front;
use crate::proxy::{bind_proxy_listener, Admission, ProxyConfig};

use super::face::{access_note, lan_line, off_line};
use super::nets::{MeshAddress, NetDirectory};
use super::ProxyFace;

/// The listener's settings as they were applied.
#[derive(Clone, PartialEq)]
struct Applied {
    enabled: bool,
    port: u16,
    access: ListenAccess,
    ifaces: IfaceFilter,
}

impl Applied {
    fn of(settings: &ProcessSettings) -> Applied {
        Applied {
            enabled: settings.socks_enabled,
            port: settings.socks_port,
            access: settings.socks_access,
            ifaces: settings.socks_ifaces.clone(),
        }
    }
}

/// A running accept task and the settings it was started with.
struct Live {
    applied: Applied,
    task: Option<JoinHandle<()>>,
}

struct Inner {
    target: watch::Sender<Option<ProxyConfig>>,
    face: watch::Sender<ProxyFace>,
    nets: NetDirectory,
    /// One limit of UDP associations for the whole process, whatever the target.
    udp_slots: Arc<tokio::sync::Semaphore>,
    /// Held by `apply` and `close`: calls go one at a time.
    live: Mutex<Option<Live>>,
}

/// The one proxy listener of the process; clones share it.
#[derive(Clone)]
pub struct ProxyFront(Arc<Inner>);

impl ProxyFront {
    /// Takes the port from the settings (nothing is taken if the proxy is off). A busy port is a `BindError`.
    pub async fn bind(settings: &ProcessSettings) -> Result<ProxyFront> {
        let front = ProxyFront(Arc::new(Inner {
            target: watch::channel(None).0,
            face: watch::channel(ProxyFace::OFF).0,
            nets: NetDirectory::default(),
            udp_slots: crate::proxy::udp_slots(),
            live: Mutex::new(None),
        }));
        let applied = Applied::of(settings);
        let task = front.listen(&applied).await?;
        *front.0.live.lock().await = Some(Live { applied, task });
        Ok(front)
    }

    /// Sets what the clients are served through; `None` — they are refused. Open connections are not touched.
    pub fn set_target(&self, cfg: Option<ProxyConfig>) {
        let cfg = cfg.map(|c| ProxyConfig {
            udp_slots: self.0.udp_slots.clone(),
            ..c
        });
        self.0.target.send_replace(cfg);
    }

    /// The shared directory of the running networks' configurations.
    pub(crate) fn nets(&self) -> NetDirectory {
        self.0.nets.clone()
    }

    /// Mesh addresses of every device the running networks list, one per device and network.
    pub fn mesh_addresses(&self) -> Vec<MeshAddress> {
        self.0.nets.mesh_addresses()
    }

    pub fn addr(&self) -> Option<SocketAddr> {
        self.0.face.borrow().addr
    }

    pub fn face(&self) -> watch::Receiver<ProxyFace> {
        self.0.face.subscribe()
    }

    /// The route context of the current target.
    pub fn current_route(&self) -> Option<RouteCtx> {
        self.0.target.borrow().as_ref().map(|c| c.route.clone())
    }

    /// Brings the listener to the settings: nothing if they are the same; another port is taken before the old one is
    /// closed (on failure the old one keeps working); the same port is closed first, and on failure there is no listener.
    pub async fn apply(&self, settings: &ProcessSettings) -> Result<()> {
        let mut live = self.0.live.lock().await;
        let applied = Applied::of(settings);
        let Some(old) = live.take() else {
            return self.start(&mut live, applied).await;
        };
        let both_off = !old.applied.enabled && !applied.enabled;
        if old.applied == applied || both_off {
            *live = Some(old);
            return Ok(());
        }
        if old.applied.enabled && applied.enabled && old.applied.port != applied.port {
            return self.replace(&mut live, old, applied).await;
        }
        self.stop(old.task).await;
        self.start(&mut live, applied).await
    }

    /// The listener is closed and its port is free when this returns.
    pub async fn close(&self) {
        let old = self.0.live.lock().await.take();
        self.stop(old.and_then(|live| live.task)).await;
        self.0.face.send_replace(ProxyFace::OFF);
    }

    /// Another port: the new listener first, then the old one is closed.
    async fn replace(&self, live: &mut Option<Live>, old: Live, applied: Applied) -> Result<()> {
        let bound = bind_proxy_listener(applied.port, applied.access).await;
        let listener = match bound {
            Ok(listener) => listener,
            Err(e) => {
                *live = Some(old);
                return Err(e);
            }
        };
        self.stop(old.task).await;
        let task = self.accept(listener, &applied);
        *live = Some(Live {
            applied,
            task: Some(task),
        });
        Ok(())
    }

    /// Starts from nothing; a failure leaves no listener.
    async fn start(&self, live: &mut Option<Live>, applied: Applied) -> Result<()> {
        self.0.face.send_replace(ProxyFace::OFF);
        if !applied.enabled {
            say("socks", off_line(false));
        }
        let task = self.listen(&applied).await?;
        *live = Some(Live { applied, task });
        Ok(())
    }

    /// Takes the port and starts accepting; with the proxy off, takes nothing.
    async fn listen(&self, applied: &Applied) -> Result<Option<JoinHandle<()>>> {
        if !applied.enabled {
            self.0.face.send_replace(ProxyFace::OFF);
            return Ok(None);
        }
        let listener = bind_proxy_listener(applied.port, applied.access).await?;
        Ok(Some(self.accept(listener, applied)))
    }

    /// Starts the accept task on a taken port, publishes the face and prints the journal lines.
    fn accept(&self, listener: tokio::net::TcpListener, applied: &Applied) -> JoinHandle<()> {
        let bound = listener.local_addr().ok();
        let port = bound.map_or(applied.port, |a| a.port());
        let adm = Admission {
            access: applied.access,
            ifaces: applied.ifaces.clone(),
        };
        let lan = lan_line(&adm, port);
        let task =
            crate::node::output::spawn(serve_front(
            listener,
            self.0.target.subscribe(),
            self.nets(),
            adm,
        ));
        self.0.face.send_replace(ProxyFace {
            addr: Some(SocketAddr::from((Ipv4Addr::LOCALHOST, port))),
            access: applied.access,
            ifaces: applied.ifaces.clone(),
        });
        let shown = bound.map_or(port.to_string(), |a| a.to_string());
        say(
            "socks",
            format!(
                "accepting connections on {shown}{}",
                access_note(applied.access)
            ),
        );
        lan.into_iter().for_each(|line| say("socks", line));
        task
    }

    /// Aborts the accept task and waits for it to end: destroying its future closes the socket.
    async fn stop(&self, task: Option<JoinHandle<()>>) {
        let Some(handle) = task else { return };
        handle.abort();
        let _ = handle.await;
    }
}

#[cfg(test)]
#[path = "listen_tests.rs"]
mod tests;
