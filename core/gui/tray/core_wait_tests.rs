//! Tests of `Core::recv_for` (`core_wait.rs`): the wait between icon attempts that still serves commands.

use super::super::super::spin::Spin;
use super::super::super::test_tray::TEST_TRAY;
use super::super::{channel, CmdTx, Core, NodeCmd, TrayCmd};
use crate::gui::tray::app::exit_menu::{disabled_exit_menu, ExitMenu};
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};
use crate::gui::tray::app::view::{NodeState, View};
use std::time::{Duration, Instant};

const SHORT: Duration = Duration::from_millis(150);
const LONG: Duration = Duration::from_secs(30);

mod quit_in_channel {
    use super::*;

    #[test]
    fn recv_for_returns_quit_code_early() {
        // given
        let mut fx = setup();
        fx.tx.send(TrayCmd::QuitWithCode(7));

        // when
        let (code, took) = fx.recv_for(LONG);

        // then
        assert_eq!(code, Some(7));
        assert!(took < Duration::from_secs(5), "returned after {took:?}");
    }
}

mod empty_channel {
    use super::*;

    #[test]
    fn recv_for_times_out_on_empty_channel() {
        // given
        let mut fx = setup();

        // when
        let (code, took) = fx.recv_for(SHORT);

        // then
        assert_eq!(code, None);
        assert!(
            took >= SHORT,
            "returned after {took:?}, before the deadline"
        );
    }
}

mod update_in_channel {
    use super::*;

    #[test]
    fn recv_for_applies_update_and_keeps_waiting() {
        // given
        let mut fx = setup();
        let view = running_view();
        fx.tx.send(TrayCmd::Update(view));

        // when
        let (code, took) = fx.recv_for(SHORT);

        // then
        assert_eq!(code, None);
        assert!(took >= SHORT, "stopped waiting after {took:?}");
        assert_eq!(fx.core.view, view, "the view must be remembered");
    }
}

mod exit_menu_in_channel {
    use super::*;

    #[test]
    fn recv_for_keeps_exit_menu_for_late_attach() {
        // given
        let mut fx = setup();
        let menu = exit_menu_serving();
        fx.tx.send(TrayCmd::ExitMenu(menu.clone()));

        // when
        let (code, _took) = fx.recv_for(SHORT);

        // then
        assert_eq!(code, None);
        assert_eq!(
            fx.core.last_exit,
            Some(menu),
            "the exit menu must be kept until the icon appears"
        );
    }
}

struct Fixture {
    tx: CmdTx,
    core: Core,
    _node_rx: tokio::sync::mpsc::UnboundedReceiver<NodeCmd>,
}

impl Fixture {
    /// Waits up to `pause` and returns what `recv_for` said and how long it took.
    fn recv_for(&mut self, pause: Duration) -> (Option<i32>, Duration) {
        let started = Instant::now();
        let code = self.core.recv_for(pause);
        (code, started.elapsed())
    }
}

fn no_link() -> Option<String> {
    None
}

/// A `Core` without a raised icon and its command sender; no real spinner ticks.
fn setup() -> Fixture {
    let (node_tx, node_rx) = tokio::sync::mpsc::unbounded_channel();
    let (tx, mut core) = channel(
        crate::gui::tray::app::quit::QuitSignal::new(),
        no_link,
        node_tx,
        &TEST_TRAY,
    );
    core.spin = Spin::with_period(Duration::from_secs(3600));
    Fixture {
        tx,
        core,
        _node_rx: node_rx,
    }
}

/// A view that differs from `View::initial()`.
fn running_view() -> View {
    View {
        node: NodeState::Running,
        ..View::initial()
    }
}

/// An exit menu that differs from the empty one by the "serves exit" flag.
fn exit_menu_serving() -> ExitMenu {
    let mut menu = disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings));
    menu.serves_exit = true;
    menu
}
