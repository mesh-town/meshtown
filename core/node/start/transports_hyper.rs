//! How the node wires in the `hyper` transport: who is admitted to the services and where the `hyperdht` source takes DHT records from.
//! Without the `net-hyper` feature — doubles: there are no records, nothing to supply.

use std::sync::Arc;

use crate::members::membership::Members;
use crate::transport::net_hyperdht::RecordStore;

use super::boot::Boot;

#[cfg(feature = "net-hyper")]
use crate::transport::net::service::{AUTH, INVITE};
#[cfg(feature = "net-hyper")]
use crate::transport::net_hyper::gate::Admit;

/// Member admission: AUTH and INVITE — for everyone, the other services — for those with a live connection
/// after AUTH. The predicate reads `Members` on every inbound connection.
#[cfg(feature = "net-hyper")]
fn member_admit(members: Members) -> Admit {
    Arc::new(move |peer, svc| *svc == AUTH || *svc == INVITE || members.is_member(peer))
}

/// The admission of the window before `admit_members_on_hyper`: only AUTH and INVITE.
#[cfg(feature = "net-hyper")]
pub(super) fn auth_invite_only() -> Admit {
    Arc::new(|_, svc| *svc == AUTH || *svc == INVITE)
}

/// The DHT record store of the same `hyper` node; `None` — there is no transport.
#[cfg(feature = "net-hyper")]
pub(super) fn hyper_records(boot: &Boot) -> Option<Arc<dyn RecordStore>> {
    boot.hyper.as_ref().map(|h| h.records())
}

/// Replaces the window's admission with the live member list.
#[cfg(feature = "net-hyper")]
pub(super) fn admit_members_on_hyper(boot: &Boot, members: &Members) {
    let Some(hyper) = &boot.hyper else { return };
    hyper.set_admit(Some(member_admit(members.clone())));
}

#[cfg(not(feature = "net-hyper"))]
pub(super) fn hyper_records(_boot: &Boot) -> Option<Arc<dyn RecordStore>> {
    None
}

#[cfg(not(feature = "net-hyper"))]
pub(super) fn admit_members_on_hyper(_boot: &Boot, _members: &Members) {}

#[cfg(all(test, feature = "net-hyper"))]
#[path = "transports_hyper_tests.rs"]
mod tests;
