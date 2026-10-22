//! Publishing a check record to the document and reading the results of all nodes: `Checks::publish` writes,
//! another reads the results of all nodes and prints them by the domains of its own list
//! (`Checks::check_views`/`print_table`/`print_new_foreign`). When to check is
//! decided by `schedule.rs`, the check itself is `crate::site_checks::reachability`.

use anyhow::{Context, Result};
use bytes::Bytes;

use crate::node::output::say;
use crate::site_checks::reachability::{self, CheckRecord, CHECK_RECORD_VERSION};
use crate::transport::net::{NetError, ReplicaEntry};

use super::describe::{describe, describe_steps};
use super::keys::{domain_from_check_key, only_listed, CheckView, CHECK_PREFIX};
use super::state::Checks;

impl Checks {
    /// Checks a domain right now and publishes the result.
    pub async fn check_now(&self, domain: &str) -> Result<CheckRecord> {
        let domain = crate::proxy::domains::normalize_domain(domain)?;
        let record =
            reachability::check_domain(&self.check_config, &self.resolver, &domain).await?;
        say("check", format!("{domain}: {}", describe_steps(&record)));
        self.publish(&domain, &record).await?;
        Ok(record)
    }

    /// Puts a record into the document under the key `chk/<domain>/`.
    pub async fn publish(&self, domain: &str, record: &CheckRecord) -> Result<()> {
        let value = postcard::to_stdvec(record).context("check record not serialized")?;
        self.shared_replica
            .set_own(&super::keys::check_key(domain), Bytes::from(value))
            .await
            .map_err(NetError::into_anyhow)
            .context("check record not published")?;
        Ok(())
    }

    /// Reads the results of all nodes: one key per domain, each node has its own author.
    pub async fn check_views(&self) -> Result<Vec<CheckView>> {
        let entries = self
            .shared_replica
            .scan(CHECK_PREFIX.as_bytes())
            .await
            .map_err(NetError::into_anyhow)?;
        let mut out = Vec::new();
        for entry in entries {
            let Some(domain) = domain_from_check_key(&entry.key) else {
                continue;
            };
            let record = record_of(&entry);
            out.push(CheckView {
                domain,
                author: entry.author,
                entry_ts_micros: entry.ts_micros,
                record,
            });
        }
        out.sort_by(|a, b| {
            a.domain
                .cmp(&b.domain)
                .then_with(|| a.author.fmt_short().cmp(&b.author.fmt_short()))
        });
        Ok(out)
    }

    /// Results only for the domains of its own list.
    async fn listed_check_views(&self) -> Result<Vec<CheckView>> {
        let list = self.domain_list.list().await?;
        Ok(only_listed(self.check_views().await?, &list))
    }

    /// How many check records this node has published — for the `[status]` line.
    pub async fn own_count(&self) -> Result<usize> {
        Ok(self
            .check_views()
            .await?
            .iter()
            .filter(|v| v.author == self.shared_replica.me())
            .count())
    }

    /// Prints a summary over all domains: its own line and the peers' lines.
    pub async fn print_table(&self) {
        let views = match self.listed_check_views().await {
            Ok(v) => v,
            Err(e) => {
                say("checks", format!("couldn't read the results: {e:#}"));
                return;
            }
        };
        let domains = {
            let mut d: Vec<String> = views.iter().map(|v| v.domain.clone()).collect();
            d.sort();
            d.dedup();
            d
        };
        say("checks", format!("domains with results: {}", domains.len()));
        for domain in domains {
            let mut parts = Vec::new();
            for view in views.iter().filter(|v| v.domain == domain) {
                let who = if view.author == self.shared_replica.me() {
                    "own".to_string()
                } else {
                    view.author.fmt_short()
                };
                let what = match &view.record {
                    Some(r) => describe(r),
                    None => "content has not arrived yet".to_string(),
                };
                parts.push(format!("{who} {what}"));
            }
            say("checks", format!("  {domain}: {}", parts.join("; ")));
        }
    }

    /// Prints new foreign results — once per record.
    pub(super) async fn print_new_foreign(&self) {
        let views = match self.listed_check_views().await {
            Ok(v) => v,
            Err(_) => return,
        };
        let mut shown = self.printed_check_keys.lock().await;
        for view in views {
            if view.author == self.shared_replica.me() {
                continue;
            }
            let key = (view.author, view.domain.clone(), view.entry_ts_micros);
            if !shown.insert(key) {
                continue;
            }
            let what = match &view.record {
                Some(r) => describe(r),
                None => "content has not arrived yet".to_string(),
            };
            say(
                "check",
                format!(
                    "{}: {} reports {}",
                    view.domain,
                    view.author.fmt_short(),
                    what
                ),
            );
        }
    }
}

/// A check record from a replica value: a tombstone, broken bytes and a foreign `v` give `None`.
fn record_of(entry: &ReplicaEntry) -> Option<CheckRecord> {
    if entry.is_tombstone() {
        return None;
    }
    postcard::from_bytes::<CheckRecord>(&entry.value)
        .ok()
        .filter(|r| r.v == CHECK_RECORD_VERSION)
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
