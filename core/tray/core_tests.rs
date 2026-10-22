//! Tests of the tray command channel and dispatcher: wakeup, exit code, menu commands (`core.rs`).

use super::*;
use crate::app::proxy_state::{ProxyOff, ProxyState};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::super::spin::Spin;

fn no_link() -> Option<String> {
    None
}

/// A command channel with a third argument; the test holds the receiver of commands to the node poller.
fn test_core() -> (CmdTx, Core, tokio::sync::mpsc::UnboundedReceiver<NodeCmd>) {
    let (node_tx, node_rx) = tokio::sync::mpsc::unbounded_channel();
    let (tx, mut core) = channel(crate::app::quit::QuitSignal::new(), no_link, node_tx);
    // No real ticks in tests: a ticker thread started by a `Starting` view sleeps for an hour; ticks are sent by hand.
    core.spin = Spin::with_period(Duration::from_secs(3600));
    (tx, core, node_rx)
}

#[test]
fn cmd_tx_wakes_after_send() {
    static WAKES: AtomicUsize = AtomicUsize::new(0);
    fn count_wake() {
        WAKES.fetch_add(1, Ordering::SeqCst);
    }

    let (sender, rx) = std::sync::mpsc::channel();
    let tx = CmdTx {
        tx: Arc::new(Mutex::new(sender)),
        wake: count_wake,
    };

    tx.send(TrayCmd::QuitWithCode(0));

    assert_eq!(WAKES.load(Ordering::SeqCst), 1);
    assert!(matches!(rx.try_recv(), Ok(TrayCmd::QuitWithCode(0))));
}

#[test]
fn drain_returns_exit_code() {
    let (tx, mut core, _node_rx) = test_core();
    tx.send(TrayCmd::QuitWithCode(7));
    assert_eq!(core.drain_commands(), Some(7));
}

#[test]
fn update_without_ui_is_ignored() {
    let (tx, mut core, _node_rx) = test_core();
    tx.send(TrayCmd::Update(crate::app::view::View::initial()));
    assert_eq!(core.drain_commands(), None);
}

#[test]
fn menu_open_without_link_is_silent() {
    let (tx, mut core, _node_rx) = test_core();
    tx.send(TrayCmd::Menu(MENU_OPEN.to_string()));
    assert_eq!(core.drain_commands(), None);
}

#[test]
fn menu_unknown_id_is_silent() {
    let (tx, mut core, mut node_rx) = test_core();
    tx.send(TrayCmd::Menu("x".to_string()));
    assert_eq!(core.drain_commands(), None);
    assert!(node_rx.try_recv().is_err());
}

/// `MENU_NODE` (instead of `MENU_START`/`MENU_STOP`): the command to the poller depends on
/// the node state in the last `TrayCmd::Update` — first the view, then the click.
mod menu_node {
    use super::*;

    #[test]
    fn absent_node_starts_it() {
        assert_eq!(
            menu_click_after_update(crate::app::view::NodeState::Absent, MENU_NODE),
            Some(NodeCmd::Start)
        );
    }

    #[test]
    fn running_node_stops_it() {
        assert_eq!(
            menu_click_after_update(crate::app::view::NodeState::Running, MENU_NODE),
            Some(NodeCmd::Stop)
        );
    }

    #[test]
    fn stopping_node_sends_nothing() {
        assert_eq!(
            menu_click_after_update(crate::app::view::NodeState::Stopping, MENU_NODE),
            None,
            "the node is already stopping — the command is not sent again"
        );
    }

    #[test]
    fn no_prior_update_sends_nothing() {
        // Before the first TrayCmd::Update, Core.view is View::initial() (NodeState::Unknown, see
        // channel()); start_enabled/stop_enabled are both false for it (view_tests.rs) — the click
        // is silent.
        let (tx, mut core, mut node_rx) = test_core();
        tx.send(TrayCmd::Menu(MENU_NODE.to_string()));
        assert_eq!(core.drain_commands(), None);
        assert!(node_rx.try_recv().is_err());
    }

    #[test]
    fn dropped_receiver_does_not_panic() {
        let (tx, mut core, node_rx) = test_core();
        drop(node_rx);
        tx.send(TrayCmd::Update(view_with_node(
            crate::app::view::NodeState::Absent,
        )));
        tx.send(TrayCmd::Menu(MENU_NODE.to_string()));
        assert_eq!(core.drain_commands(), None);
    }
}

/// `MENU_STATUS` (the status item) and `MENU_PROXY` (the proxy in the tray) — both
/// are inactive and a click on them sends nothing to the node poller.
mod menu_status_and_proxy {
    use super::*;

    #[test]
    fn menu_status_click_sends_nothing() {
        assert_eq!(
            menu_click_after_update(crate::app::view::NodeState::Running, MENU_STATUS),
            None
        );
    }

    #[test]
    fn menu_proxy_click_sends_nothing() {
        assert_eq!(
            menu_click_after_update(crate::app::view::NodeState::Running, MENU_PROXY),
            None
        );
    }
}

#[test]
fn menu_exit_id_toggles_that_exit() {
    let (tx, mut core, mut node_rx) = test_core();
    tx.send(TrayCmd::Menu(format!("{MENU_EXIT_PREFIX}abcd1234")));
    assert_eq!(core.drain_commands(), None);
    assert_eq!(
        node_rx.try_recv(),
        Ok(NodeCmd::ToggleExit("abcd1234".to_string()))
    );
}

#[test]
fn menu_exit_prefix_alone_clears_exits() {
    let (tx, mut core, mut node_rx) = test_core();
    tx.send(TrayCmd::Menu(MENU_EXIT_PREFIX.to_string()));
    assert_eq!(core.drain_commands(), None);
    assert_eq!(node_rx.try_recv(), Ok(NodeCmd::ClearExits));
}

#[test]
fn menu_invite_sends_copy_invite() {
    let (tx, mut core, mut node_rx) = test_core();
    tx.send(TrayCmd::Menu(MENU_INVITE.to_string()));
    assert_eq!(core.drain_commands(), None);
    assert_eq!(node_rx.try_recv(), Ok(NodeCmd::CopyInvite));
}

#[test]
fn invite_note_without_ui_is_ignored() {
    use crate::app::tray_actions::{InviteNote, InviteReach};
    let (tx, mut core, _node_rx) = test_core();
    tx.send(TrayCmd::Invite(InviteNote::Copied(InviteReach::Spread)));
    assert_eq!(core.drain_commands(), None);
}

#[test]
fn menu_serve_asks_opposite_of_snapshot() {
    for serves in [false, true] {
        let (tx, mut core, mut node_rx) = test_core();
        let mut menu = crate::app::exit_menu::disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings));
        menu.serves_exit = serves;
        tx.send(TrayCmd::ExitMenu(menu));
        tx.send(TrayCmd::Menu(MENU_SERVE.to_string()));
        assert_eq!(core.drain_commands(), None);
        assert_eq!(node_rx.try_recv(), Ok(NodeCmd::SetExitServer(!serves)));
    }
}

/// A view with the given node state; the color and the page are plausible values, `handle_menu`
/// looks only at `node`.
fn view_with_node(node: crate::app::view::NodeState) -> crate::app::view::View {
    crate::app::view::View {
        color: crate::app::view::TrayColor::Grey,
        page: crate::app::view::PageState::Ready,
        node,
        ring: crate::app::view_ring::Ring::EMPTY,
        disk: None,
        net: crate::app::view::NetLink::NoNetwork,
    }
}

/// Sends `TrayCmd::Update(a view with this node)`, then a click on `id`; returns the first command
/// to the node poller, if one was sent.
fn menu_click_after_update(node: crate::app::view::NodeState, id: &str) -> Option<NodeCmd> {
    let (tx, mut core, mut node_rx) = test_core();
    tx.send(TrayCmd::Update(view_with_node(node)));
    tx.send(TrayCmd::Menu(id.to_string()));
    assert_eq!(core.drain_commands(), None);
    node_rx.try_recv().ok()
}

#[test]
fn serve_note_without_ui_is_ignored() {
    let (tx, mut core, _node_rx) = test_core();
    let failed = ServeNote::Failed {
        was_enabling: true,
        reason: "the node did not accept the change",
    };
    tx.send(TrayCmd::Serve(failed));
    assert_eq!(core.drain_commands(), None);
    assert_eq!(core.serve_note, failed);
}

/// The gray "no node" view, as the poller sends it (`tray_role::views::ABSENT_VIEW` is not visible from here).
fn absent_view() -> crate::app::view::View {
    use crate::app::view::{NodeState, PageState, TrayColor, View};
    View {
        color: TrayColor::Grey,
        page: PageState::Unavailable,
        node: NodeState::Absent,
        ..View::initial()
    }
}

fn busy_line() -> String {
    crate::app::start_note::start_note_line(&StartNote::ProxyBusy(1080)).unwrap()
}

#[test]
fn start_fail_then_absent_shows_reason() {
    let (tx, mut core, _node_rx) = test_core();
    tx.send(TrayCmd::StartFail(StartNote::ProxyBusy(1080)));
    tx.send(TrayCmd::Update(absent_view()));
    assert_eq!(core.drain_commands(), None);
    assert_eq!(core.menu_first_line(), busy_line());
}

#[test]
fn idle_note_restores_stopped_line() {
    let (tx, mut core, _node_rx) = test_core();
    tx.send(TrayCmd::StartFail(StartNote::ProxyBusy(1080)));
    tx.send(TrayCmd::Update(absent_view()));
    tx.send(TrayCmd::StartFail(StartNote::Idle));
    assert_eq!(core.drain_commands(), None);
    assert_eq!(
        core.menu_first_line(),
        "Mesh Town is stopped — sites won't open through it"
    );
}

#[test]
fn start_note_hidden_until_absent() {
    use crate::app::view::NodeState;
    let (tx, mut core, _node_rx) = test_core();
    let starting = crate::app::view::View {
        node: NodeState::Starting,
        ..absent_view()
    };
    tx.send(TrayCmd::Update(starting));
    tx.send(TrayCmd::StartFail(StartNote::ProxyBusy(1080)));
    assert_eq!(core.drain_commands(), None);
    assert_eq!(core.menu_first_line(), "Mesh Town is starting…");

    tx.send(TrayCmd::Update(absent_view()));
    assert_eq!(core.drain_commands(), None);
    assert_eq!(core.menu_first_line(), busy_line());
}

#[test]
fn start_fail_reason_shows_in_first_line() {
    let (tx, mut core, _node_rx) = test_core();
    let note = StartNote::FailedWith("key is not set".into());
    tx.send(TrayCmd::StartFail(note.clone()));
    tx.send(TrayCmd::Update(absent_view()));
    assert_eq!(core.drain_commands(), None);
    assert_eq!(core.menu_first_line(), crate::app::start_note::start_note_line(&note).unwrap());
}

/// The spinner of a starting node and the grey ring of a stopped one, as the tray icon sees them
/// (`Core::icon_key`): the white sector moves on `SpinTick`, only while `Starting` lasts.
mod spinner {
    use super::*;
    use crate::app::view::NodeState;
    use crate::app::view_ring::{Ring, RingRole};

    #[test]
    fn starting_tick_moves_white_sector() {
        let (tx, mut core, _node_rx) = test_core();
        update(&tx, &mut core, view_with_node(NodeState::Starting));
        assert_eq!(core.icon_key().lit, Some(0));

        tx.send(TrayCmd::SpinTick(core.spin.generation()));
        assert_eq!(core.drain_commands(), None);

        assert_eq!(core.icon_key().lit, Some(1));
    }

    #[test]
    fn stuck_start_with_unavailable_page_does_not_spin() {
        let (tx, mut core, _node_rx) = test_core();
        let stuck = crate::app::view::View {
            page: crate::app::view::PageState::Unavailable,
            ..view_with_node(NodeState::Starting)
        };
        update(&tx, &mut core, view_with_node(NodeState::Starting));
        let generation = core.spin.generation();
        update(&tx, &mut core, stuck);

        tx.send(TrayCmd::SpinTick(generation));
        assert_eq!(core.drain_commands(), None);

        assert_eq!(core.icon_key().lit, None, "no white sector on a stuck start");
        assert_eq!(core.icon_key().ring, None, "the icon is solid");
        update(&tx, &mut core, view_with_node(NodeState::Starting));
        assert_ne!(core.spin.generation(), generation, "the ticker starts again when the page returns");
    }

    #[test]
    fn starting_after_running_spins_the_remembered_count() {
        let (tx, mut core, _node_rx) = test_core();
        update(&tx, &mut core, running_with_five_members());
        update(&tx, &mut core, absent_view());

        update(&tx, &mut core, view_with_node(NodeState::Starting));

        assert_eq!(core.icon_key().ring, Some(Ring::uniform(RingRole::Offline, 5)));
        for _ in 0..4 {
            tick(&tx, &mut core);
        }
        assert_eq!(core.icon_key().lit, Some(4), "the last of five sectors");
        tick(&tx, &mut core);
        assert_eq!(core.icon_key().lit, Some(0), "the white sector wraps around the ring");
    }

    #[test]
    fn tick_after_running_is_ignored() {
        let (tx, mut core, _node_rx) = test_core();
        update(&tx, &mut core, view_with_node(NodeState::Starting));
        update(&tx, &mut core, running_with_five_members());
        let before = core.spin.idle();

        tick(&tx, &mut core);

        assert_eq!(core.spin.idle(), before, "a late tick does not move the spinner");
        assert_eq!(core.icon_key().lit, None, "a running icon has no white sector");
    }

    #[test]
    fn tick_of_previous_start_is_ignored() {
        let (tx, mut core, _node_rx) = test_core();
        update(&tx, &mut core, view_with_node(NodeState::Starting));
        let old_generation = core.spin.generation();
        update(&tx, &mut core, absent_view());
        update(&tx, &mut core, view_with_node(NodeState::Starting));

        tx.send(TrayCmd::SpinTick(old_generation));
        assert_eq!(core.drain_commands(), None);

        assert_eq!(core.icon_key().lit, Some(0), "the new start is still at its first frame");
    }

    #[test]
    fn stopped_after_running_ring_shows_its_count() {
        let (tx, mut core, _node_rx) = test_core();
        update(&tx, &mut core, running_with_five_members());

        update(&tx, &mut core, absent_view());

        let key = core.icon_key();
        assert_eq!(key.ring, Some(Ring::uniform(RingRole::Offline, 5)));
        assert_eq!(key.lit, None, "a stopped node does not spin");
    }
}

/// Sends a view to the core and handles it.
fn update(tx: &CmdTx, core: &mut Core, view: crate::app::view::View) {
    tx.send(TrayCmd::Update(view));
    assert_eq!(core.drain_commands(), None);
}

/// Sends a spinner tick of the current generation and handles it.
fn tick(tx: &CmdTx, core: &mut Core) {
    tx.send(TrayCmd::SpinTick(core.spin.generation()));
    assert_eq!(core.drain_commands(), None);
}

/// A running view whose ring has five members of different roles.
fn running_with_five_members() -> crate::app::view::View {
    use crate::app::view_ring::{Ring, RingRole};
    let ring = Ring::EMPTY
        .add(RingRole::Online)
        .add(RingRole::Online)
        .add(RingRole::ActiveExit)
        .add(RingRole::ExitClient)
        .add(RingRole::Offline);
    crate::app::view::View { ring, ..view_with_node(crate::app::view::NodeState::Running) }
}
