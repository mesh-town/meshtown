//! Page actions on domains: exit search, checking one/all, adding and removing.
//! Bodies of the `NodeControl for NodeCtl` methods; the trait block in `control.rs` only delegates here.

use crate::node::output::say;
use crate::proxy::domains;
use crate::web_gui::ActionError;

use super::state::NodeCtl;

impl NodeCtl {
    pub(super) fn find_exit_impl(&self, domain: &str) -> Result<(), ActionError> {
        let node = self.running_node_for_action()?;
        let name = domains::normalize_domain(domain)
            .map_err(|e| ActionError::Invalid(format!("couldn't parse the domain name: {e:#}")))?;
        if !node.checks.try_begin_check(&name) {
            return Err(ActionError::Busy(format!("{name}: check already running")));
        }
        self.ctl_shared.kick.notify_one();
        let ctl_shared = self.ctl_shared.clone();
        crate::node::output::spawn_on(&self.rt, async move {
            if let Err(e) = node.domains.add(&name).await {
                say("node", format!("{name}: couldn't add to the list — {e:#}"));
            }
            if let Err(e) = node.checks.check_now(&name).await {
                say("node", format!("{name}: check failed — {e:#}"));
            }
            node.checks.end_check(&name);
            ctl_shared.kick.notify_one();
        });
        Ok(())
    }

    pub(super) fn check_domain_impl(&self, domain: &str) -> Result<(), ActionError> {
        let node = self.running_node_for_action()?;
        let known = self.known_domains();
        let name = domains::normalize_domain(domain)
            .ok()
            .filter(|n| known.contains(n));
        let Some(name) = name else {
            return Err(ActionError::NotFound(format!("{domain}: domain not found")));
        };
        if !node.checks.try_begin_check(&name) {
            return Err(ActionError::Busy(format!("{name}: check already running")));
        }
        self.ctl_shared.kick.notify_one();
        let ctl_shared = self.ctl_shared.clone();
        crate::node::output::spawn_on(&self.rt, async move {
            if let Err(e) = node.checks.check_now(&name).await {
                say("node", format!("{name}: check failed — {e:#}"));
            }
            node.checks.end_check(&name);
            ctl_shared.kick.notify_one();
        });
        Ok(())
    }

    pub(super) fn check_all_impl(&self) -> Result<u64, ActionError> {
        let node = self.running_node_for_action()?;
        let known = self.known_domains();
        let busy = node.checks.domains_in_progress();
        if known.iter().any(|d| busy.contains(d)) {
            return Err(ActionError::Busy(
                "a check is already running for one or more domains".to_string(),
            ));
        }
        let taken: Vec<String> = known
            .into_iter()
            .filter(|d| node.checks.try_begin_check(d))
            .collect();
        let count = taken.len() as u64;
        if !taken.is_empty() {
            self.ctl_shared.kick.notify_one();
            let ctl_shared = self.ctl_shared.clone();
            crate::node::output::spawn_on(&self.rt, async move {
                for name in taken {
                    if let Err(e) = node.checks.check_now(&name).await {
                        say("node", format!("{name}: check failed — {e:#}"));
                    }
                    node.checks.end_check(&name);
                    ctl_shared.kick.notify_one();
                }
            });
        }
        Ok(count)
    }

    pub(super) fn add_domain_impl(&self, domain: &str) -> Result<String, ActionError> {
        let node = self.running_node_for_action()?;
        let name = domains::normalize_domain(domain)
            .map_err(|e| ActionError::Invalid(format!("couldn't parse the domain name: {e:#}")))?;
        {
            let mut st = self
                .ctl_shared
                .ctl_state
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(c) = &mut st.cache {
                if !c.domains.contains(&name) {
                    c.domains.push(name.clone());
                }
            }
        }
        let task_name = name.clone();
        let ctl_shared = self.ctl_shared.clone();
        crate::node::output::spawn_on(&self.rt, async move {
            if let Err(e) = node.domains.add(&task_name).await {
                say(
                    "node",
                    format!("{task_name}: couldn't add to the list — {e:#}"),
                );
                return;
            }
            // The "For you" column is filled right away, as with "find an exit"; if busy, the scheduled cycle does it.
            if node.checks.try_begin_check(&task_name) {
                if let Err(e) = node.checks.check_now(&task_name).await {
                    say("node", format!("{task_name}: check failed — {e:#}"));
                }
                node.checks.end_check(&task_name);
                ctl_shared.kick.notify_one();
            }
        });
        Ok(name)
    }

    pub(super) fn remove_domain_impl(&self, domain: &str) -> Result<String, ActionError> {
        let node = self.running_node_for_action()?;
        let name = domains::normalize_domain(domain)
            .map_err(|e| ActionError::Invalid(format!("couldn't parse the domain name: {e:#}")))?;
        {
            let mut st = self
                .ctl_shared
                .ctl_state
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(c) = &mut st.cache {
                c.domains.retain(|d| d != &name);
            }
        }
        let task_name = name.clone();
        crate::node::output::spawn_on(&self.rt, async move {
            if let Err(e) = node.domains.remove(&task_name).await {
                say(
                    "node",
                    format!("{task_name}: couldn't remove from the list — {e:#}"),
                );
            }
        });
        Ok(name)
    }
}
