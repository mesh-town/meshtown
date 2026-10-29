use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::autostart::backend::{AutostartBackend, EntryState};

const ENTRY_TEXT: &str = "[Desktop Entry]\nName=Test\n";
const SWITCHED_OFF_TEXT: &str = "[Desktop Entry]\nName=Test\nHidden=true\n";

fn text_is_hidden(text: &str) -> bool {
    text.contains("Hidden=true")
}

#[test]
fn enable_creates_dirs_and_file() {
    // given: a home without the entry directory
    let fx = setup("enable_creates_dirs_and_file");
    assert!(!fx.home().exists());
    // when
    fx.entry.enable().unwrap();
    // then
    assert_eq!(fs::read_to_string(&fx.entry.path).unwrap(), ENTRY_TEXT);
    assert_eq!(fx.entry.state(), Ok(EntryState::Current));
}

#[test]
fn enable_overwrites_stale() {
    // given: the file holds someone else's text
    let fx = setup("enable_overwrites_stale");
    fx.write_existing("[Desktop Entry]\nName=Old\n");
    assert_eq!(fx.entry.state(), Ok(EntryState::Stale));
    // when
    fx.entry.enable().unwrap();
    // then
    assert_eq!(fx.entry.state(), Ok(EntryState::Current));
    assert_eq!(
        fx.names_beside_entry(),
        vec![fx.entry.path.file_name().unwrap().to_owned()],
        "no temporary file is left behind"
    );
}

#[test]
fn disable_removes_file() {
    // given
    let fx = setup("disable_removes_file");
    fx.write_existing(ENTRY_TEXT);
    // when
    fx.entry.disable().unwrap();
    // then
    assert!(!fx.entry.path.exists());
    assert_eq!(fx.entry.state(), Ok(EntryState::Absent));
}

#[test]
fn disable_absent_is_ok() {
    // given: no file
    let fx = setup("disable_absent_is_ok");
    // when / then
    assert_eq!(fx.entry.disable(), Ok(()));
    assert_eq!(fx.entry.state(), Ok(EntryState::Absent));
}

#[test]
fn enable_into_file_parent_fails() {
    // given: the directory that should hold the entry is a regular file
    let fx = setup("enable_into_file_parent_fails");
    fs::create_dir_all(fx.home()).unwrap();
    fs::write(fx.entry.path.parent().unwrap(), "not a directory").unwrap();
    // when
    let err = fx.entry.enable().unwrap_err();
    // then
    let shown = fx.entry.path.display().to_string();
    assert!(err.contains(&shown), "error `{err}` should name {shown}");
}

#[test]
fn switched_off_text_is_reported() {
    // given
    let fx = setup("switched_off_text_is_reported");
    fx.write_existing(SWITCHED_OFF_TEXT);
    // when / then
    assert_eq!(fx.entry.state(), Ok(EntryState::SwitchedOff));
}

#[test]
fn unreadable_entry_is_error() {
    // given: a directory where the entry file should be
    let fx = setup("unreadable_entry_is_error");
    fs::create_dir_all(&fx.entry.path).unwrap();
    // when / then
    assert!(fx.entry.state().is_err(), "a read failure is not `Absent`");
}

#[test]
fn trait_methods_act_on_the_file() {
    // given
    let fx = setup("trait_methods_act_on_the_file");
    let backend: &dyn AutostartBackend = &fx.entry;
    // when
    backend.enable().unwrap();
    let on = backend.state();
    backend.disable().unwrap();
    // then
    assert_eq!(on, Ok(EntryState::Current));
    assert_eq!(backend.state(), Ok(EntryState::Absent));
}

struct Fixture {
    root: PathBuf,
    entry: FileEntry,
}

impl Fixture {
    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    fn write_existing(&self, text: &str) {
        fs::create_dir_all(self.entry.path.parent().unwrap()).unwrap();
        fs::write(&self.entry.path, text).unwrap();
    }

    fn names_beside_entry(&self) -> Vec<std::ffi::OsString> {
        let dir = self.entry.path.parent().unwrap();
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn setup(name: &str) -> Fixture {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root =
        std::env::temp_dir().join(format!("meshtown-autostart-{}-{id}-{name}", process::id()));
    let _ = fs::remove_dir_all(&root);
    let path: &Path = &root.join("home").join("autostart").join("meshtown.desktop");
    let entry = FileEntry {
        path: path.to_path_buf(),
        text: ENTRY_TEXT.to_string(),
        switched_off: text_is_hidden,
    };
    Fixture { root, entry }
}
