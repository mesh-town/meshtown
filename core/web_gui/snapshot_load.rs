//! Snapshot structs for the exit-overload fields: the owner's load figures and the client's local sign.

/// The exit's own load, for `exit.load`; `null` while the exit of this network is off.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct ExitLoadInfo {
    /// Streams open now.
    pub streams: u64,
    /// The cap in force.
    pub limit: u64,
    /// The cap by machine memory (what the setting 0 gives).
    pub limit_auto: u64,
    /// Why the exit is overloaded (`limit`, `share`, `memory`), while it is.
    pub overloaded: Option<String>,
    /// Streams admitted over the last ten minutes (the whole process).
    pub admitted: u64,
    /// Streams refused over the last ten minutes (the whole process).
    pub refused: u64,
    /// Members of this network with refusals, most refused first, at most 20.
    pub members: Vec<ExitLoadMember>,
    /// Refusals not made to members of this network (the counter is one per process).
    pub elsewhere_refused: u64,
}

/// One member's share of the exit's decisions over ten minutes.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct ExitLoadMember {
    pub id_full: String,
    pub admitted: u64,
    pub refused: u64,
}

/// What this client's own connections to one exit saw over the last two minutes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct TurnedAway {
    pub refused: u64,
    pub attempts: u64,
}
