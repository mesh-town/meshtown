//! The baseline of "waiting for a restart" follows the node's `boot_id`: `sync_boot_baseline`, `seed_boot_id`.
//! The harness is from `mod_tests.rs` via `use super::*;`.

use super::*;
use crate::config::config_file::ListenAccess;

/// A state whose file asks for the wider proxy access while the node still runs the narrow one.
fn state_asking_for_wider_access(dir: &Path) -> ConfigState {
    let mut state = new_state(dir);
    state.file_settings.process.socks_access = ListenAccess::Localnet;
    state.boot_file_settings.process.socks_access = ListenAccess::Localhost;
    state
}

#[test]
fn a_restart_moves_the_baseline_to_the_file() {
    // given: the id of the first lap is known and a change waits for a restart
    let dir = temp_dir();
    let mut state = state_asking_for_wider_access(&dir);
    state.seed_boot_id("aaaaaaaa");
    assert_eq!(state.pending_restart_paths(), vec!["socks.access"]);
    // when: the node restarts before anything has read its state
    state.sync_boot_baseline("bbbbbbbb");
    // then: the file is what the node runs now
    assert!(state.pending_restart_paths().is_empty());
    cleanup(&dir);
}

#[test]
fn without_a_seed_the_first_look_moves_nothing() {
    // given: no id is known yet, so a restart before the first look cannot be told from the start
    let dir = temp_dir();
    let mut state = state_asking_for_wider_access(&dir);
    // when
    state.sync_boot_baseline("bbbbbbbb");
    // then: the change stays pending, the id is only remembered
    assert_eq!(state.pending_restart_paths(), vec!["socks.access"]);
    assert_eq!(state.last_boot_id.as_deref(), Some("bbbbbbbb"));
    cleanup(&dir);
}

#[test]
fn a_seed_does_not_replace_a_known_id() {
    // given: the id of the second lap is known
    let dir = temp_dir();
    let mut state = new_state(&dir);
    state.seed_boot_id("aaaaaaaa");
    state.sync_boot_baseline("bbbbbbbb");
    // when
    state.seed_boot_id("cccccccc");
    // then
    assert_eq!(state.last_boot_id.as_deref(), Some("bbbbbbbb"));
    cleanup(&dir);
}
