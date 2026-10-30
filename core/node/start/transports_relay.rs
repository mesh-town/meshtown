//! How the node wires in relay through network members: the candidate source for dials and the advert of its own
//! relay. Without the `net-hyper` feature there is no transport, so nothing is wired.

use std::sync::Arc;

use tokio::task::JoinHandle;

use crate::config::config_file::RelayMode;
use crate::members::access::view::AccessView;
use crate::members::membership::Members;
use super::relay_advert::{advertise_relay, retract_relay};
use crate::node::start::relay_candidates::relay_source;
use crate::transport::net::Replica;
use crate::transport::net_hyper::{HyperTransport, RelayAdmit};

/// `Off`: no source and the own advert is retracted. `Network`: the source reads the adverts of the network's
/// replica and the own relay, once it runs, is advertised there. The task goes to the node's background handles.
pub(super) fn wire_relay(
    hyper: &Arc<HyperTransport>,
    mode: RelayMode,
    members: &Members,
    replica: Arc<dyn Replica>,
) -> JoinHandle<()> {
    match mode {
        RelayMode::Off => retract_task(hyper, replica),
        RelayMode::Network => advertise_task(hyper, members, replica),
    }
}

fn retract_task(hyper: &Arc<HyperTransport>, replica: Arc<dyn Replica>) -> JoinHandle<()> {
    hyper.set_relay_source(None);
    crate::node::output::spawn(async move {
        if let Err(e) = retract_relay(&*replica).await {
            tracing::warn!("relay: retracting the advert failed: {e}");
        }
    })
}

fn advertise_task(
    hyper: &Arc<HyperTransport>,
    members: &Members,
    replica: Arc<dyn Replica>,
) -> JoinHandle<()> {
    let source = relay_source(members.clone(), replica.clone(), replica.me());
    hyper.set_relay_source(Some(source));
    crate::node::output::spawn(advertise_relay(hyper.serve_relay(admit_of(members.access())), replica))
}

/// Admission for the own relay: only nodes the network's current verdicts allow; read afresh at every pairing.
fn admit_of(access: AccessView) -> RelayAdmit {
    Arc::new(move |peer| access.allows(peer))
}

#[cfg(all(test, feature = "net-hyper"))]
#[path = "transports_relay_tests.rs"]
mod tests;
