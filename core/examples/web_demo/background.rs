//! Background task of the demo node: advances model time, writes the log and status, and takes the checks.

use std::sync::Arc;
use std::time::Duration;

use meshtown_core::web_gui::{NodeStatus, VerdictKind};

use crate::node::{DemoNode, DemoState};
use crate::rng::xorshift64;

/// Returns a new log line if model time has reached `next_line_at`: moves the timer and
/// picks a random tag.
fn next_log_line(st: &mut DemoState, t: f64) -> Option<(String, String)> {
    if t < st.next_line_at {
        return None;
    }
    let gap = 0.5 + (xorshift64(&mut st.gen) % 1000) as f64 / 1000.0;
    st.next_line_at = t + gap;
    let tags = ["node", "dht", "member", "gossip", "socks", "check"];
    let tag = tags[(xorshift64(&mut st.gen) % tags.len() as u64) as usize];
    Some((tag.to_string(), format!("{tag}: event t={t:.1}")))
}

/// A `[status]` line once per model minute.
fn next_status_line(st: &mut DemoState, t: f64) -> Option<String> {
    if t < st.next_status_at {
        return None;
    }
    st.next_status_at = t + 60.0;
    Some("status: no changes".to_string())
}

/// Clears `Starting` after `switch_network`, once model time has reached `starting_until`.
fn clear_expired_starting(st: &mut DemoState, t: f64) {
    let Some(until) = st.starting_until else {
        return;
    };
    if t >= until {
        st.status = NodeStatus::Online;
        st.starting_until = None;
    }
}

fn roll_check_verdict(gen: &mut u64) -> VerdictKind {
    let roll = xorshift64(gen) % 100;
    if roll < 70 {
        VerdictKind::Ok
    } else if roll < 80 {
        VerdictKind::DnsMismatch
    } else if roll < 90 {
        VerdictKind::SiteGeoblock
    } else {
        VerdictKind::NetworkBlocked
    }
}

/// Finishes the domain checks whose `checking_until` has arrived, with a random verdict.
fn resolve_due_domain_checks(st: &mut DemoState, t: f64) {
    let due: Vec<usize> = st
        .domains
        .iter()
        .enumerate()
        .filter(|(_, d)| d.checking_until.is_some_and(|u| t >= u))
        .map(|(i, _)| i)
        .collect();
    for i in due {
        let verdict = roll_check_verdict(&mut st.gen);
        st.domains[i].checking_until = None;
        st.domains[i].last_verdict = Some(verdict);
    }
}

/// One tick: advances `t`, decides on the log/status, clears `Starting`, finishes overdue
/// domain checks. The log lines are returned to the caller so they can be written outside the lock.
fn advance_demo_tick(demo: &DemoNode) -> (f64, Option<(String, String)>, Option<String>) {
    let mut st = demo.state.lock().unwrap();
    st.t += 0.2 * st.fast;
    let t = st.t;
    let line = next_log_line(&mut st, t);
    let status_line = next_status_line(&mut st, t);
    clear_expired_starting(&mut st, t);
    resolve_due_domain_checks(&mut st, t);
    (t, line, status_line)
}

fn emit_tick_log(
    demo: &DemoNode,
    t: f64,
    line: Option<(String, String)>,
    status_line: Option<String>,
) {
    if let Some((tag, text)) = line {
        demo.push_log(t, &tag, text);
    }
    if let Some(text) = status_line {
        demo.push_log(t, "status", text);
    }
}

/// Background task: every 200 ms of real time it advances model time by `0.2 * fast`, from time
/// to time adds a line to the log, once per model minute adds a `[status]` line, finishes
/// overdue domain checks, and clears `Starting` after `switch_network`.
pub(crate) async fn run_background(demo: Arc<DemoNode>) {
    let mut interval = tokio::time::interval(Duration::from_millis(200));
    loop {
        interval.tick().await;
        let (t, line, status_line) = advance_demo_tick(&demo);
        emit_tick_log(&demo, t, line, status_line);
    }
}
