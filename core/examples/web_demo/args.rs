//! Command-line flags of the demo node (`Args`) and the `FileState`/`IdentityMode` enums.

use clap::{Parser, ValueEnum};

use crate::managed::ManagedMode;

// ============================================================================================
// Flags
// ============================================================================================

#[derive(Parser, Debug)]
#[command(
    name = "web_demo",
    about = "Demo node for manual and automated checks of the page"
)]
pub(crate) struct Args {
    #[arg(long, default_value_t = 7777)]
    pub(crate) port: u16,
    /// Fixed token instead of a random one (for acceptance scripts).
    #[arg(long)]
    pub(crate) token: Option<String>,
    /// Seed of the demo data generator; 0 is treated as 1.
    #[arg(long, default_value_t = 1)]
    pub(crate) seed: u64,
    /// Multiplier of the model-time rate (one model second = 200 ms real time / fast).
    #[arg(long, default_value_t = 1.0)]
    pub(crate) fast: f64,
    /// Corrupts the settings file in a given way (for checking file.state/file.message).
    #[arg(long, value_enum)]
    pub(crate) file_state: Option<FileState>,
    /// path[=value] — overrides a settings field from the CLI, like a real node; repeatable.
    #[arg(long = "cli-override")]
    pub(crate) cli_override: Vec<String>,
    /// Do not create a network, peers or domains — a demo without a connected network.
    #[arg(long)]
    pub(crate) no_network: bool,
    /// How many of the two demo peers (counting from the end of the list) to report as offline (`online: false`).
    #[arg(long, default_value_t = 0)]
    pub(crate) offline_peers: u64,
    /// `transport.dht.joined` in the snapshot (demo-only, for web-check).
    #[arg(long)]
    pub(crate) dht_joined: bool,
    /// `transport.dht.nodes` in the snapshot (demo-only, for web-check); meaningful with `--dht-joined`.
    #[arg(long, default_value_t = 0)]
    pub(crate) dht_nodes: u64,
    #[arg(long, value_enum, default_value_t = IdentityMode::Persistent)]
    pub(crate) identity: IdentityMode,
    /// path — corrupts this settings field with a broken value from the table (repeatable).
    #[arg(long = "field-error")]
    pub(crate) field_error: Vec<String>,
    /// Capacity of the log ring; by default it is log.ring_lines from the settings file.
    #[arg(long)]
    pub(crate) ring_lines: Option<usize>,
    /// Another member agrees to every ban proposed from this page at once (the member lands in "No longer in the network").
    #[arg(long)]
    pub(crate) members_second_vote: bool,
    /// Start as a computer already banned from the network (`me_excluded` in the member list).
    #[arg(long)]
    pub(crate) me_excluded: bool,
    /// A node that asked to join waits in the list for confirmations (no member has confirmed it yet).
    #[arg(long)]
    pub(crate) members_pending: bool,
    /// Start as a computer that asked to join and waits for confirmations (`me_pending` in the member list).
    #[arg(long)]
    pub(crate) me_pending: bool,
    /// Offline members can be removed from the list: adds rows that were not seen for days and a removed one.
    #[arg(long)]
    pub(crate) members_forget: bool,
    /// Start as a computer the other members removed from the list (the card is shown in the `main` network only).
    #[arg(long)]
    pub(crate) me_removed: bool,
    /// The network is managed and this node created it: it is an administrator and the root.
    #[arg(long, conflicts_with = "managed_member")]
    pub(crate) managed: bool,
    /// The network is managed and this node is an ordinary member; two other members are administrators.
    #[arg(long)]
    pub(crate) managed_member: bool,
    /// How many networks the page shows (1..=8); the extra ones are labelled `demo-network-2`, ...
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=8))]
    pub(crate) nets: u8,
    /// Do not start the page at all — everything else is prepared as usual.
    #[arg(long)]
    pub(crate) no_web: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
#[value(rename_all = "kebab-case")]
pub(crate) enum FileState {
    Invalid,
    Unreadable,
    Readonly,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
#[value(rename_all = "snake_case")]
pub(crate) enum IdentityMode {
    Persistent,
    Declined,
    FileProblem,
    FilePreserved,
}

impl Args {
    pub(crate) fn managed_mode(&self) -> ManagedMode {
        match (self.managed, self.managed_member) {
            (true, _) => ManagedMode::Admin,
            (_, true) => ManagedMode::Member,
            _ => ManagedMode::Peer,
        }
    }
}
