//! Finding the settings file on disk (`CONFIG-FILE.md`, section 1) and the real process environment.
//! What isn't here: parsing and writing the file itself (`parse.rs`, `render.rs`, `save.rs`).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{DIR_NAME, FILE_NAME};

/// The operating system, for choosing the standard file location.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    /// Linux and other non-macOS systems.
    Unix,
}

/// Everything the path choice depends on. Tests build it by hand.
pub struct PathEnv<'a> {
    pub os: Os,
    /// Environment variable reader; an empty value means not set.
    pub var: &'a dyn Fn(&str) -> Option<OsString>,
    pub exe_dir: Option<PathBuf>,
    pub is_root: bool,
    pub is_file: &'a dyn Fn(&Path) -> bool,
}

/// What the settings file source selected.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfigSource {
    /// `--no-config`: do not read or write the file.
    Disabled,
    /// The path is chosen.
    File {
        path: PathBuf,
        may_create: bool,
        rule: PathRule,
    },
    /// No location: no rule gave a path (no HOME/APPDATA). The reason is given as text.
    Nowhere(String),
}

/// Which rule of CONFIG-FILE.md section 1 fired.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathRule {
    Explicit,
    NextToBinary,
    Standard,
}

/// The variable's value; an empty one counts as not set (a safeguard on top of `PathEnv::var`).
fn env_var(env: &PathEnv, name: &str) -> Option<OsString> {
    (env.var)(name).filter(|v| !v.is_empty())
}

/// The order of CONFIG-FILE.md section 1: `explicit` (the `--config` flag or `MESHTOWN_CONFIG`, clap has already merged them)
/// → `<binary directory>/config.json` if that file exists → the standard location.
pub fn resolve_config_source(explicit: Option<&Path>, env: &PathEnv) -> ConfigSource {
    if let Some(p) = explicit {
        return ConfigSource::File {
            path: p.to_path_buf(),
            may_create: true,
            rule: PathRule::Explicit,
        };
    }
    if let Some(d) = &env.exe_dir {
        let beside = d.join(FILE_NAME);
        if (env.is_file)(&beside) {
            return ConfigSource::File {
                path: beside,
                may_create: false,
                rule: PathRule::NextToBinary,
            };
        }
    }
    match standard_path(env) {
        Ok(p) => ConfigSource::File {
            path: p,
            may_create: true,
            rule: PathRule::Standard,
        },
        Err(why) => ConfigSource::Nowhere(why),
    }
}

/// Rule 4: the system's standard location, the path `<base>/meshtown/config.json`.
pub fn standard_path(env: &PathEnv) -> Result<PathBuf, String> {
    let base: PathBuf = match env.os {
        Os::Windows => PathBuf::from(
            env_var(env, "APPDATA").ok_or_else(|| "the APPDATA variable is not set".to_string())?,
        ),
        Os::MacOs => PathBuf::from(
            env_var(env, "HOME").ok_or_else(|| "the HOME variable is not set".to_string())?,
        )
        .join("Library")
        .join("Application Support"),
        Os::Unix if env.is_root => PathBuf::from("/etc"),
        Os::Unix => {
            // XDG_CONFIG_HOME with a relative path is ignored (the XDG specification requires this).
            // `has_root` rather than `is_absolute`: a Unix-style `/x` is rooted on every host OS.
            let xdg = env_var(env, "XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .filter(|p| p.has_root());
            match xdg {
                Some(p) => p,
                None => PathBuf::from(
                    env_var(env, "HOME").ok_or_else(|| "the HOME variable is not set".to_string())?,
                )
                .join(".config"),
            }
        }
    };
    Ok(base.join(DIR_NAME).join(FILE_NAME))
}

/// Environment variable reader: name → value.
type VarReader = dyn Fn(&str) -> Option<OsString>;

/// The real process environment. Holds the closures in a `Box`; `PathEnv` borrows from it.
pub struct CurrentEnv {
    os: Os,
    var: Box<VarReader>,
    exe_dir: Option<PathBuf>,
    is_root: bool,
    is_file: Box<dyn Fn(&Path) -> bool>,
}

impl CurrentEnv {
    pub fn as_path_env(&self) -> PathEnv<'_> {
        PathEnv {
            os: self.os,
            var: &*self.var,
            exe_dir: self.exe_dir.clone(),
            is_root: self.is_root,
            is_file: &*self.is_file,
        }
    }
}

/// The real environment: `cfg!(windows)`/`cfg!(target_os = "macos")`, `std::env::var_os`,
/// the directory of `std::env::current_exe()`, `is_root_linux()`, `|p| p.is_file()`.
pub fn current_env() -> CurrentEnv {
    let os = if cfg!(windows) {
        Os::Windows
    } else if cfg!(target_os = "macos") {
        Os::MacOs
    } else {
        Os::Unix
    };
    CurrentEnv {
        os,
        var: Box::new(|name| std::env::var_os(name).filter(|v| !v.is_empty())),
        exe_dir: std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf)),
        is_root: is_root_linux(),
        is_file: Box::new(|p| p.is_file()),
    }
}

/// The effective uid is zero (Linux, `/proc/self/status`, the `Uid:` line, the second number).
/// A read error gives `false`.
#[cfg(target_os = "linux")]
pub fn is_root_linux() -> bool {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return false;
    };
    status
        .lines()
        .find_map(|l| l.strip_prefix("Uid:"))
        .and_then(|rest| rest.split_whitespace().nth(1))
        .and_then(|euid| euid.parse::<u32>().ok())
        == Some(0)
}

/// On other systems it is always `false`.
#[cfg(not(target_os = "linux"))]
pub fn is_root_linux() -> bool {
    false
}

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
