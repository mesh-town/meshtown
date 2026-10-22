//! Exit refusal: the reply code for the member and the tail of the log line.

use crate::exit::frame::{EXIT_CODE_ADDRESS_FORBIDDEN, EXIT_CODE_NO_ADDRESS};

/// The reply code for the member and the tail of the log line (`refuse` adds the short id).
#[derive(Debug)]
pub(in crate::exit) struct Refusal {
    pub code: u8,
    pub line: String,
}

impl Refusal {
    pub(super) fn no_address(line: String) -> Refusal {
        Refusal {
            code: EXIT_CODE_NO_ADDRESS,
            line,
        }
    }

    /// There is one reason and no address: an address from DoH is given out neither to the member nor to the log.
    pub(super) fn forbidden(target_host: &str) -> Refusal {
        Refusal {
            code: EXIT_CODE_ADDRESS_FORBIDDEN,
            line: format!("{target_host} — address closed to exit"),
        }
    }
}
