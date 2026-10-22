use super::*;
use crate::config_file::{DIR_NAME, FILE_NAME};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The environment for path tests: the variables as a list, a file next to the binary present or not.
struct TestEnv {
    os: Os,
    vars: Vec<(&'static str, &'static str)>,
    exe_dir: Option<PathBuf>,
    is_root: bool,
    file_beside: bool,
}

impl TestEnv {
    fn new(os: Os) -> TestEnv {
        TestEnv {
            os,
            vars: vec![],
            exe_dir: None,
            is_root: false,
            file_beside: false,
        }
    }

    fn var(mut self, name: &'static str, value: &'static str) -> TestEnv {
        self.vars.push((name, value));
        self
    }

    fn resolve(&self, explicit: Option<&Path>) -> ConfigSource {
        let vars = self.vars.clone();
        let var = move |k: &str| -> Option<OsString> {
            vars.iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| OsString::from(*v))
        };
        let file_beside = self.file_beside;
        let is_file = move |_: &Path| file_beside;
        let env = PathEnv {
            os: self.os,
            var: &var,
            exe_dir: self.exe_dir.clone(),
            is_root: self.is_root,
            is_file: &is_file,
        };
        resolve_config_source(explicit, &env)
    }
}

fn standard(t: &TestEnv) -> ConfigSource {
    t.resolve(None)
}

fn std_file(path: PathBuf) -> ConfigSource {
    ConfigSource::File {
        path,
        may_create: true,
        rule: PathRule::Standard,
    }
}

#[test]
fn explicit_wins() {
    let mut t = TestEnv::new(Os::MacOs).var("HOME", "/home/x");
    t.exe_dir = Some(PathBuf::from("/opt/mesh"));
    t.file_beside = true;
    let got = t.resolve(Some(Path::new("/custom/c.json")));
    assert_eq!(
        got,
        ConfigSource::File {
            path: PathBuf::from("/custom/c.json"),
            may_create: true,
            rule: PathRule::Explicit,
        }
    );
}

#[test]
fn next_to_binary_only_if_exists() {
    let mut t = TestEnv::new(Os::MacOs).var("HOME", "/home/x");
    t.exe_dir = Some(PathBuf::from("/opt/mesh"));
    t.file_beside = true;
    assert_eq!(
        t.resolve(None),
        ConfigSource::File {
            path: PathBuf::from("/opt/mesh").join(FILE_NAME),
            may_create: false,
            rule: PathRule::NextToBinary,
        }
    );
    t.file_beside = false;
    assert_eq!(
        t.resolve(None),
        std_file(
            PathBuf::from("/home/x")
                .join("Library")
                .join("Application Support")
                .join(DIR_NAME)
                .join(FILE_NAME)
        )
    );
}

#[test]
fn standard_windows() {
    let t = TestEnv::new(Os::Windows).var("APPDATA", r"D:\Profile\AppData\Roaming");
    assert_eq!(
        standard(&t),
        std_file(
            PathBuf::from(r"D:\Profile\AppData\Roaming")
                .join(DIR_NAME)
                .join(FILE_NAME)
        )
    );
}

#[test]
fn standard_macos() {
    let t = TestEnv::new(Os::MacOs).var("HOME", "/home/x");
    assert_eq!(
        standard(&t),
        std_file(
            PathBuf::from("/home/x")
                .join("Library")
                .join("Application Support")
                .join(DIR_NAME)
                .join(FILE_NAME)
        )
    );
}

#[test]
fn standard_linux_root() {
    let mut t = TestEnv::new(Os::Unix);
    t.is_root = true;
    assert_eq!(
        standard(&t),
        std_file(PathBuf::from("/etc").join(DIR_NAME).join(FILE_NAME))
    );
}

#[test]
fn standard_linux_xdg() {
    let t = TestEnv::new(Os::Unix)
        .var("XDG_CONFIG_HOME", "/x")
        .var("HOME", "/h");
    assert_eq!(
        standard(&t),
        std_file(PathBuf::from("/x").join(DIR_NAME).join(FILE_NAME))
    );
}

#[test]
fn standard_linux_xdg_relative_ignored() {
    let t = TestEnv::new(Os::Unix)
        .var("XDG_CONFIG_HOME", "rel")
        .var("HOME", "/h");
    assert_eq!(
        standard(&t),
        std_file(
            PathBuf::from("/h")
                .join(".config")
                .join(DIR_NAME)
                .join(FILE_NAME)
        )
    );
}

#[test]
fn standard_linux_home() {
    let t = TestEnv::new(Os::Unix).var("HOME", "/h");
    assert_eq!(
        standard(&t),
        std_file(
            PathBuf::from("/h")
                .join(".config")
                .join(DIR_NAME)
                .join(FILE_NAME)
        )
    );
}

#[test]
fn nowhere_without_home() {
    let t = TestEnv::new(Os::MacOs);
    match standard(&t) {
        ConfigSource::Nowhere(why) => assert!(why.contains("HOME"), "reason: {why}"),
        other => panic!("expected Nowhere, got {other:?}"),
    }
}
