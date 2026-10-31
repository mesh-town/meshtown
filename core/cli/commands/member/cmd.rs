//! The command line of the `member` group: the parsed types.

/// `member` and its action.
#[derive(clap::Args)]
pub(crate) struct MemberArgs {
    #[command(subcommand)]
    pub(crate) cmd: MemberCmd,
}

#[derive(clap::Subcommand)]
pub(crate) enum MemberCmd {
    /// List the members of the network: who is in it, who waits for admission, who is online
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown member list [--net <NETWORK>] [--json]")]
    List,
    /// Admit a member who waits for admission (administrators only)
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown member admit <MEMBER> [--net <NETWORK>] [--json]")]
    Admit {
        /// The member: the name (any letter case), or the start of the id from `member list` (at least 4 digits).
        #[arg(value_name = "MEMBER", help_heading = "Options")]
        member: String,
    },
    /// Remove a member from the network for everyone (administrators only)
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown member exclude <MEMBER> [--reason <TEXT>] [--net <NETWORK>] [--json]")]
    Exclude {
        /// The member: the name (any letter case), or the start of the id from `member list` (at least 4 digits).
        #[arg(value_name = "MEMBER", help_heading = "Options")]
        member: String,
        /// Why the member is removed, up to 80 characters; the removed member sees it.
        #[arg(long, value_name = "TEXT")]
        reason: Option<String>,
    },
    /// Withdraw the removal of a member (administrators only)
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown member unexclude <MEMBER> [--net <NETWORK>] [--json]")]
    Unexclude {
        /// The member: the name (any letter case), or the start of the id from `member list` (at least 4 digits).
        #[arg(value_name = "MEMBER", help_heading = "Options")]
        member: String,
    },
    /// Refuse connections with a member on this computer only
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown member block <MEMBER> [--net <NETWORK>] [--json]")]
    Block {
        /// The member: the name (any letter case), or the start of the id from `member list` (at least 4 digits).
        #[arg(value_name = "MEMBER", help_heading = "Options")]
        member: String,
    },
    /// Allow connections with a blocked member again
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown member unblock <MEMBER> [--net <NETWORK>] [--json]")]
    Unblock {
        /// The member: the name (any letter case), or the start of the id from `member list` (at least 4 digits).
        #[arg(value_name = "MEMBER", help_heading = "Options")]
        member: String,
    },
    /// Make a member an administrator, or take the role away (administrators only)
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown member admin <MEMBER> [--revoke] [--net <NETWORK>] [--json]")]
    Admin {
        /// The member: the name (any letter case), or the start of the id from `member list` (at least 4 digits).
        #[arg(value_name = "MEMBER", help_heading = "Options")]
        member: String,
        /// Take the administrator role away instead of giving it.
        #[arg(long)]
        revoke: bool,
    },
}

impl MemberCmd {
    /// The member the action is about; `None` for `list`.
    pub(crate) fn member(&self) -> Option<&str> {
        match self {
            MemberCmd::List => None,
            MemberCmd::Admit { member }
            | MemberCmd::Exclude { member, .. }
            | MemberCmd::Unexclude { member }
            | MemberCmd::Block { member }
            | MemberCmd::Unblock { member }
            | MemberCmd::Admin { member, .. } => Some(member),
        }
    }

    /// The word of the command and of the route `members/<word>`.
    pub(crate) fn verb(&self) -> &'static str {
        match self {
            MemberCmd::List => "list",
            MemberCmd::Admit { .. } => "admit",
            MemberCmd::Exclude { .. } => "exclude",
            MemberCmd::Unexclude { .. } => "unexclude",
            MemberCmd::Block { .. } => "block",
            MemberCmd::Unblock { .. } => "unblock",
            MemberCmd::Admin { .. } => "admin",
        }
    }
}
