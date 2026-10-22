//! Node identity state: whether the secret is kept between runs, and the text for the node and the page.

use crate::config_file::FileReport;

/// Why the node identity is not kept between runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemporaryReason {
    Declined,
    FilePreserved,
    FileProblem,
}

/// Node identity state: whether the secret is kept between runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityState {
    Persistent,
    Temporary(TemporaryReason),
}

impl IdentityState {
    /// Builds the identity state from the config file report; the checks run in a fixed order.
    pub fn from_report(report: &FileReport) -> Self {
        if report.persistent_identity {
            Self::Persistent
        } else if report.identity_declined {
            Self::Temporary(TemporaryReason::Declined)
        } else if report.overwrite_refused {
            Self::Temporary(TemporaryReason::FilePreserved)
        } else {
            Self::Temporary(TemporaryReason::FileProblem)
        }
    }

    pub fn kind_code(self) -> &'static str {
        match self {
            Self::Persistent => "persistent",
            Self::Temporary(_) => "temporary",
        }
    }

    pub fn temporary_reason_code(self) -> Option<&'static str> {
        match self {
            Self::Persistent => None,
            Self::Temporary(TemporaryReason::Declined) => Some("declined"),
            Self::Temporary(TemporaryReason::FilePreserved) => Some("file_preserved"),
            Self::Temporary(TemporaryReason::FileProblem) => Some("file_problem"),
        }
    }

    pub fn kind_word(self) -> &'static str {
        match self {
            Self::Persistent => "persistent",
            Self::Temporary(_) => "temporary",
        }
    }

    pub fn warning_text(self) -> Option<String> {
        match self {
            Self::Persistent => None,
            Self::Temporary(TemporaryReason::Declined) => Some(
                "Node identity is not saved, as set at startup. \
After a restart, peers will see a new node."
                    .to_string(),
            ),
            Self::Temporary(TemporaryReason::FilePreserved) => Some(format!(
                "The file on disk is not broken and was left as is on purpose: rewriting it would mean losing \
the person's data, and a backup copy could not be made. There is no need to delete this file — it may contain the \
{} network key. After a restart, peers will see a new node.",
                crate::output::NET_NAME
            )),
            Self::Temporary(TemporaryReason::FileProblem) => Some(
                "Node identity is not saved because of a problem with the config file. \
After a restart, peers will see a new node."
                    .to_string(),
            ),
        }
    }
}

#[cfg(test)]
#[path = "identity_tests.rs"]
mod tests;
