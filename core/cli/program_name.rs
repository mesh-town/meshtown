//! The name the program was started under, for hints that tell a person which command to type.
//! What it does: remembers the first command line argument (`set_program_name`) and gives its file stem
//! (`program_name`): `meshtown-cli` for the build without a tray, `meshtown` for the build with it; only a `.exe`
//! extension (in any case) is dropped, so a name with a version such as `meshtown-cli-0.3.50` stays whole. What it doesn't do: it doesn't parse arguments or check that the name is a real command.

use std::ffi::OsStr;
use std::path::Path;
use std::sync::OnceLock;

/// The name used in hints when the program was not started under another one (and in tests).
pub const DEFAULT_PROGRAM_NAME: &str = "meshtown";

static PROGRAM_NAME: OnceLock<String> = OnceLock::new();

/// Remembers how the program was started (the first argument). An empty or non-UTF-8 name falls back to
/// `meshtown`. Only the first call has an effect.
pub fn set_program_name(argv0: &OsStr) {
    let _ = PROGRAM_NAME.set(name_from_argv0(argv0));
}

/// The name of the command for hints; `meshtown` until `set_program_name` is called.
pub fn program_name() -> &'static str {
    PROGRAM_NAME
        .get()
        .map_or(DEFAULT_PROGRAM_NAME, String::as_str)
}

/// The file name of the first argument without a `.exe` extension, or `meshtown` if there is none.
fn name_from_argv0(argv0: &OsStr) -> String {
    let file = Path::new(argv0).file_name().and_then(OsStr::to_str);
    match file.map(without_exe) {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => DEFAULT_PROGRAM_NAME.to_string(),
    }
}

fn without_exe(file: &str) -> &str {
    let cut = file.len().saturating_sub(".exe".len());
    match file.get(cut..) {
        Some(tail) if tail.eq_ignore_ascii_case(".exe") => &file[..cut],
        _ => file,
    }
}

#[cfg(test)]
#[path = "program_name_tests.rs"]
mod tests;
