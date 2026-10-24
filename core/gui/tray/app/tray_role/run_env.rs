//! Building the tray poller environment (`build_poll_env`) and launching it in the background (`spawn_poller`);
//! the helpers (`child_args`, `proxy_plan_for`, `spawn_child`, `say`) come from the parent `run`.

use std::path::PathBuf;
use std::time::Duration;

use super::*;

/// Fallback exit: how long to wait after `QuitWithCode` before `exit_now`.
const EXIT_GRACE: Duration = Duration::from_secs(5);

/// The poller environment: port and token, child launch, showing in the tray via `send`, quit signals.
pub(super) fn build_poll_env(
    target: (u16, PathBuf),
    (settings, args): (&crate::config::config_file::Settings, &TrayArgs),
    cmds: tokio::sync::mpsc::UnboundedReceiver<NodeCmd>,
    quit: QuitSignal,
    send: impl Fn(TrayCmd) + Clone + Send + 'static,
    tray: &'static dyn crate::gui::tray::TrayBackend,
) -> PollEnv {
    let (port, token_path) = target;
    let argv = child_args(args);
    let proxy_plan = proxy_plan_for(
        settings.process.socks_port,
        settings.process.socks_enabled,
        &args.run_args,
    );
    let access = proxy_access_for(settings.process.socks_access, &args.run_args);
    let (show_tx, show_exits_tx, show_invite_tx) = (send.clone(), send.clone(), send.clone());
    let (show_serve_tx, start_tx) = (send.clone(), send);
    let quit_wait = quit.clone();
    let quit_on_signal = quit;
    PollEnv {
        port,
        token_path,
        web_enabled: settings.process.web_enabled,
        proxy: ProxyState::of_plan(proxy_plan),
        spawn: Box::new(move || spawn_child(&argv)),
        send_view: super::super::first_run::view_sender(show_tx),
        send_exit_menu: Box::new(move |m| show_exits_tx(TrayCmd::ExitMenu(m))),
        copy_to_clipboard_fn: std::sync::Arc::new(move |text: &str| tray.copy_to_clipboard(text)),
        notify_fn: std::sync::Arc::new(move |n| {
            crate::gui::tray::notify::spawn_notify(tray, n);
        }),
        send_invite_note: Box::new(move |n| show_invite_tx(TrayCmd::Invite(n))),
        send_serve_note: Box::new(move |n| show_serve_tx(TrayCmd::Serve(n))),
        start_fail: StartFail::new(
            Box::new(move || busy_proxy_port(proxy_plan.ok(), access)),
            Box::new(crate::cli::os_console::node_log_tail),
            Box::new(move |n| start_tx(TrayCmd::StartFail(n))),
        ),
        log: Box::new(say),
        cmds,
        quit: Box::pin(async move { quit_wait.wait().await }),
        signal: Box::pin(async move {
            wait_signal().await;
            // The 15 s watchdog is armed on the signal path too.
            quit_on_signal.request();
        }),
    }
}

/// The poller in the background; its code gives the tray a `QuitWithCode`, and the fallback exit finishes off the process.
pub(super) fn spawn_poller(env: PollEnv, tx: impl Fn(TrayCmd) + Send + 'static) {
    crate::node::output::spawn_on(&tokio::runtime::Handle::current(), async move {
        let code = poll_node(env, PollTiming::PROD).await;
        tx(TrayCmd::QuitWithCode(code));
        tokio::time::sleep(EXIT_GRACE).await;
        exit_now(code);
    });
}
