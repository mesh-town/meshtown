//! The schedule loop: once per `TICK` it checks whether checks are enabled and, if so, who is due
//! (`Checks::due`/`schedule.rs`), checks (`crate::reachability`) and publishes (`store.rs`).

use std::time::Duration;

use crate::output::say;
use crate::reachability;

use super::describe::describe_steps;
use super::state::Checks;

/// One step of the schedule loop.
const TICK: Duration = Duration::from_secs(10);

/// The schedule loop: always started; once every ten seconds it checks whether checks are
/// enabled and, if so, who is due, checks and publishes.
pub async fn run_checks(checks: Checks) {
    say(
        "checks",
        format!(
            "checks {}: DoH {}, interval {} s, first check of a new domain within {} s",
            if checks.checks_enabled() { "enabled" } else { "disabled" },
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
        for domain in domains {
            if !checks.due(&domain).await {
                continue;
            }
            if !checks.try_begin_check(&domain) {
                continue;
            }
            match reachability::check_domain(&checks.check_config, &checks.resolver, &domain).await {
                Ok(record) => {
                    say("check", format!("{domain}: {}", describe_steps(&record)));
                    if let Err(e) = checks.publish(&domain, &record).await {
                        say("check", format!("{domain}: record not published: {e:#}"));
                    }
                    checks.schedule_after_verdict(&domain, record.verdict).await;
                }
                Err(e) => {
                    say("check", format!("{domain}: check failed: {e:#}"));
                    checks.postpone(&domain).await;
                }
            }
            checks.end_check(&domain);
        }
        checks.print_new_foreign().await;
    }
}
