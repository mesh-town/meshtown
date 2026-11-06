//! The schedule loop: once per `TICK` it checks whether checks are enabled and, if so, who is due
//! (`Checks::due`/`schedule.rs`), checks directly and through the exits of online members
//! (`crate::site_checks::reachability`, `exits.rs`) and publishes the direct result (`store.rs`).

use std::time::Duration;

use crate::node::output::say;
use crate::site_checks::reachability;
use crate::transport::net::PeerId;

use super::describe::describe_steps;
use super::state::Checks;
use super::target::Target;

/// One step of the schedule loop.
const TICK: Duration = Duration::from_secs(10);

/// Checks one due domain directly, then publishes the record or postpones the next attempt.
async fn check_one(checks: &Checks, domain: &str) {
    match reachability::check_domain(&checks.check_config, &checks.resolver, domain).await {
        Ok(record) => {
            say("check", format!("{domain}: {}", describe_steps(&record)));
            if let Err(e) = checks.publish(domain, &record).await {
                say("check", format!("{domain}: record not published: {e:#}"));
            }
            checks
                .schedule_after_verdict(domain, Target::Direct, record.verdict)
                .await;
        }
        Err(e) => {
            say("check", format!("{domain}: check failed: {e:#}"));
            checks.postpone(domain, Target::Direct).await;
        }
    }
}

/// The targets of the domain that are due now: directly and through each of the exits.
async fn due_targets(checks: &Checks, domain: &str, exits: &[PeerId]) -> Vec<Target> {
    let all = std::iter::once(Target::Direct).chain(exits.iter().map(|p| Target::Exit(*p)));
    let mut due = Vec::new();
    for target in all {
        if checks.due(domain, target).await {
            due.push(target);
        }
    }
    due
}

/// Starts the due exits' checks in the background (not waited for, so a slow exit holds neither the loop
/// nor the domain's "being checked" mark), then checks directly, if due, under the mark.
async fn run_targets(checks: &Checks, domain: &str, targets: Vec<Target>) {
    let mut direct = false;
    for target in targets {
        match target {
            Target::Exit(peer) => {
                checks.spawn_via_exit(domain, peer);
            }
            Target::Direct => direct = true,
        }
    }
    if direct && checks.try_begin_check(domain) {
        check_one(checks, domain).await;
        checks.end_check(domain);
    }
}

/// The schedule loop: always started; once every ten seconds it checks whether checks are
/// enabled and, if so, who is due, checks and publishes.
pub async fn run_checks(checks: Checks) {
    say(
        "checks",
        format!(
            "checks {}: DoH {}, interval {} s, first check of a new domain within {} s",
            if checks.checks_enabled() {
                "enabled"
            } else {
                "disabled"
            },
            checks.doh_server(),
            checks.schedule.base_interval().as_secs(),
            (super::schedule::FIRST_DELAY + super::schedule::FIRST_SPREAD).as_secs()
        ),
    );
    let mut tick = tokio::time::interval(TICK);
    loop {
        tick.tick().await;
        if !checks.checks_enabled() {
            continue;
        }
        let domains = match checks.domain_list.list().await {
            Ok(d) => d,
            Err(e) => {
                say("checks", format!("couldn't read the domain list: {e:#}"));
                continue;
            }
        };
        let exits = checks.current_exit_targets().await;
        for domain in domains {
            let due = due_targets(&checks, &domain, &exits).await;
            if due.is_empty() {
                continue;
            }
            run_targets(&checks, &domain, due).await;
        }
        checks.print_new_foreign().await;
    }
}
