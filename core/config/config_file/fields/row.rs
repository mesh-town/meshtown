//! A field table row: the path, the page mode, the secret flag and the value kind.

use super::value::FieldValue;

/// How a field behaves in the API and on restart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Mode {
    /// Changed from the page, applied without a restart.
    Live,
    /// Changed from the page, takes effect after a restart, published in `restart_fields`.
    RestartShown,
    /// Changed from the page, with no separate apply mode.
    EditOnly,
    /// The page only shows it.
    ReadOnly,
    /// Lives only in the file.
    FileOnly,
}

impl Mode {
    /// Returns whether the page can change the field.
    pub(crate) fn is_editable(self) -> bool {
        match self {
            Mode::Live | Mode::RestartShown | Mode::EditOnly => true,
            Mode::ReadOnly | Mode::FileOnly => false,
        }
    }
}

/// One settings field.
pub(crate) struct Field {
    /// The "section.key" path.
    pub field_path: &'static str,
    pub mode: Mode,
    /// The value is not shown in error messages.
    pub redacted: bool,
    /// How the value is read from the file.
    pub value: &'static dyn FieldValue,
}

impl std::fmt::Debug for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field")
            .field("field_path", &self.field_path)
            .field("mode", &self.mode)
            .field("redacted", &self.redacted)
            .finish_non_exhaustive()
    }
}

impl Field {
    /// Returns the part of the path before the dot.
    pub(crate) fn section(&self) -> &'static str {
        self.field_path
            .split_once('.')
            .map_or(self.field_path, |(s, _)| s)
    }

    /// Returns the part of the path after the dot.
    pub(crate) fn key(&self) -> &'static str {
        self.field_path.split_once('.').map_or("", |(_, k)| k)
    }
}

/// A table row: `row!("path", Mode, value)`, with a secret — `row!(redacted "path", …)`.
macro_rules! row {
    (redacted $path:literal, $mode:ident, $value:expr) => {
        Field {
            field_path: $path,
            mode: Mode::$mode,
            redacted: true,
            value: $value,
        }
    };
    ($path:literal, $mode:ident, $value:expr) => {
        Field {
            field_path: $path,
            mode: Mode::$mode,
            redacted: false,
            value: $value,
        }
    };
}
pub(crate) use row;
