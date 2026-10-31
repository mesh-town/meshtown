//! The `socks.*` baseline follows the listener the node reports: `sync_proxy_baseline`.
//! The harness is from `mod_tests.rs` via `use super::*;`.

use super::*;
use crate::config::config_file::{IfaceFilter, ListenAccess};
use crate::proxy::ProxyFace;
use crate::web_gui::test_host::SnapNode;

const LISTENER_PORT: u16 = 41001;
const FILE_PORT: u16 = 41002;

fn listening(access: ListenAccess) -> ProxyFace {
    ProxyFace {
        addr: Some(([127, 0, 0, 1], LISTENER_PORT).into()),
        access,
        ifaces: IfaceFilter::default(),
    }
}

/// A node with a listener face and the given boot id.
fn node_with(face: ProxyFace, boot_id: &str) -> std::sync::Arc<SnapNode> {
    let node = SnapNode::new(NodeSnapshot::default());
    node.snapshot.lock().unwrap().boot_id = boot_id.to_string();
    node.set_applied_proxy(Some(face));
    node
}

/// A state whose file narrows the access and moves the port of a listener that kept the old settings.
fn state_against_listener(dir: &Path) -> ConfigState {
    let mut state = new_state(dir);
    let file = &mut state.file_settings.process;
    file.socks_access = ListenAccess::Localhost;
    file.socks_port = FILE_PORT;
    state.boot_file_settings.process = file.clone();
    state
}

#[test]
fn socks_pending_follows_the_listener_across_laps() {
    // given: the file says localhost on B while the listener is localnet on A
    let dir = temp_dir();
    let mut state = state_against_listener(&dir);
    let node = node_with(listening(ListenAccess::Localnet), "aaaaaaaa");
    // when/then: before and after a lap the file is still pending against the listener
    state.sync_with_node(&*node);
    assert_eq!(
        state.pending_restart_paths(),
        vec!["socks.port", "socks.access"]
    );
    node.snapshot.lock().unwrap().boot_id = "bbbbbbbb".to_string();
    state.sync_with_node(&*node);
    assert_eq!(
        state.pending_restart_paths(),
        vec!["socks.port", "socks.access"]
    );
    cleanup(&dir);
}

#[test]
fn off_listener_marks_enabled_pending() {
    // given: the file asks for the proxy while the listener is off
    let dir = temp_dir();
    let mut state = new_state(&dir);
    state.file_settings.process.socks_enabled = true;
    state.boot_file_settings.process.socks_enabled = true;
    let node = node_with(ProxyFace::OFF, "aaaaaaaa");
    // when
    state.sync_with_node(&*node);
    // then
    assert_eq!(state.pending_restart_paths(), vec!["socks.enabled"]);
    cleanup(&dir);
}

#[test]
fn launch_flag_keeps_the_launch_baseline() {
    // given: the port came from a launch flag
    let dir = temp_dir();
    let mut state = state_against_listener(&dir);
    state.cli_override_paths = vec!["socks.port"];
    let node = node_with(listening(ListenAccess::Localnet), "aaaaaaaa");
    // when
    state.sync_with_node(&*node);
    // then: the listener does not move the baseline
    assert_eq!(state.boot_file_settings.process.socks_port, FILE_PORT);
    assert_eq!(
        state.boot_file_settings.process.socks_access,
        ListenAccess::Localhost
    );
    cleanup(&dir);
}
