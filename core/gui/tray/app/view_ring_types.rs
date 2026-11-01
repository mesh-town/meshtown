//! The ring's value types: member roles, their colors and the per-role counts.
//!
//! What it does: defines `RingRole`, `Ring` (how many members are in each role), the role order `RING_ROLES` and
//! the sector color `role_rgb`. Everything here is a leaf: it knows nothing about the node snapshot.
//! What it does not do: it does not read a snapshot (`view_ring.rs` does) and does not draw (`view_ring_paint.rs` does).
//! Links: `view_ring` re-exports these names; `view_ring_paint` and the tray icon sheet example use them directly.

/// A member's role in the ring. The order of the variants is the index into `Ring::counts` and the order of the sectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingRole {
    /// The selected exit is unavailable (`route.state == unavailable`).
    ExitDown,
    /// The exit our sites go through.
    ActiveExit,
    /// The exit our sites go through, reached through another member (`path == "relay"`): the
    /// colour of `ActiveExit`, drawn dashed.
    ExitViaRelay,
    /// A member that goes out to the network through us.
    ExitClient,
    /// Online.
    Online,
    /// Offline.
    Offline,
}

/// All roles in the order of the variants.
pub(super) const RING_ROLES: [RingRole; RING_ROLE_COUNT] = [
    RingRole::ExitDown,
    RingRole::ActiveExit,
    RingRole::ExitViaRelay,
    RingRole::ExitClient,
    RingRole::Online,
    RingRole::Offline,
];

/// The number of roles — the length of `RING_ROLES` and `Ring::counts`.
pub(super) const RING_ROLE_COUNT: usize = 6;

/// The sector color of a role — the only `match` on `RingRole` in the code.
pub(super) fn role_rgb(role: RingRole) -> [u8; 3] {
    match role {
        // The same red as the solid icon `TrayColor::Red` (`view_icon.rs`).
        RingRole::ExitDown => [225, 60, 50],
        // Light blue: on a dark menu row the contrast is about 6:1 (it was about 3.6:1), on a light one about 2:1 — no paler than
        // the green neighbour (1.8:1); from violet ΔE is about 38.
        RingRole::ActiveExit | RingRole::ExitViaRelay => [90, 170, 255],
        RingRole::ExitClient => [175, 75, 205],
        RingRole::Online => [60, 200, 90],
        RingRole::Offline => [140, 140, 140],
    }
}

impl RingRole {
    /// The sector is drawn as a dashed line (the path goes through another member).
    pub(super) fn is_dashed(self) -> bool {
        self == RingRole::ExitViaRelay
    }
}

/// How many members are in each role (by `RingRole as usize`). It does not remember the order of peers in the snapshot:
/// equality does not depend on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ring {
    pub(super) counts: [u16; RING_ROLE_COUNT],
}

impl Ring {
    /// An empty ring: the icon stays solid.
    pub const EMPTY: Ring = Ring {
        counts: [0; RING_ROLE_COUNT],
    };

    /// A ring with one more member in role `role`; the counter saturates at `u16::MAX`.
    pub fn add(self, role: RingRole) -> Ring {
        let mut counts = self.counts;
        counts[role as usize] = counts[role as usize].saturating_add(1);
        Ring { counts }
    }

    /// How many members are in the role.
    pub fn count(&self, role: RingRole) -> u16 {
        self.counts[role as usize]
    }

    /// Total members (sectors, counted).
    pub fn len(&self) -> usize {
        self.counts.iter().map(|&n| usize::from(n)).sum()
    }

    /// No members at all.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The role of the `k`-th member in sector order; `k` past the end gives `Offline`, with no panic.
    pub fn role_at(&self, k: usize) -> RingRole {
        let mut end = 0;
        RING_ROLES
            .into_iter()
            .find(|&role| {
                end += usize::from(self.count(role));
                k < end
            })
            .unwrap_or(RingRole::Offline)
    }
}
