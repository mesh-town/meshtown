//! `impl NodeControl for DemoNode`: the snapshot (one line through `snapshot.rs`) and the page actions.

use meshtown_core::invite::InviteHead;
use meshtown_core::transport::net::PeerAddr;

use meshtown_core::config::config_file::Settings;
use meshtown_core::web_gui::{
    ActionError, LogPage, MemberActionReq, MemberFuture, NetRoles, NodeControl, NodeSnapshot,
    NodeStatus, SaveKey,
};

use crate::invites::{enter, invite_for, key_of_node};
use crate::node::{DemoDomain, DemoNode};
use crate::rng::{gen_hex8, xorshift64};
use crate::snapshot::demo_snapshot;

impl NodeControl for DemoNode {
    fn snapshot(&self) -> NodeSnapshot {
        demo_snapshot(self)
    }

    fn log_after(&self, after: u64, limit: usize, net: Option<&str>) -> LogPage {
        let boot_id = self.state.lock().unwrap().boot_id.clone();
        self.log
            .lock()
            .unwrap()
            .page(&boot_id, after, limit.max(1), net)
    }

    fn net_roles(&self) -> Option<NetRoles> {
        self.roles_of_node()
    }

    fn members(&self) -> MemberFuture {
        let reply = self.members_list();
        Box::pin(async move { reply })
    }

    fn member_action(&self, act: MemberActionReq) -> MemberFuture {
        let reply = self.members_act(&act);
        Box::pin(async move { reply })
    }

    fn find_exit(&self, domain: &str) -> Result<(), ActionError> {
        let mut st = self.state.lock().unwrap();
        if !st.domains.iter().any(|d| d.name == domain) {
            st.domains.push(DemoDomain {
                name: domain.to_string(),
                checking_until: None,
                last_verdict: None,
            });
        }
        let t = st.t;
        let gap = 2.0 + (xorshift64(&mut st.gen) % 3000) as f64 / 1000.0;
        if let Some(d) = st.domains.iter_mut().find(|d| d.name == domain) {
            if d.checking_until.is_none() {
                d.checking_until = Some(t + gap);
            }
        }
        Ok(())
    }

    fn check_domain(&self, domain: &str) -> Result<(), ActionError> {
        let mut st = self.state.lock().unwrap();
        let t = st.t;
        let gap = 2.0 + (xorshift64(&mut st.gen) % 3000) as f64 / 1000.0;
        match st.domains.iter_mut().find(|d| d.name == domain) {
            Some(d) if d.checking_until.is_some() => Err(ActionError::Busy(format!(
                "{domain}: check already running"
            ))),
            Some(d) => {
                d.checking_until = Some(t + gap);
                Ok(())
            }
            None => Err(ActionError::NotFound(format!("{domain}: domain not found"))),
        }
    }

    fn check_all(&self) -> Result<u64, ActionError> {
        let mut st = self.state.lock().unwrap();
        if st.domains.iter().any(|d| d.checking_until.is_some()) {
            return Err(ActionError::Busy(
                "a check is already running for one or more domains".to_string(),
            ));
        }
        let t = st.t;
        let count = st.domains.len() as u64;
        let gaps: Vec<f64> = (0..st.domains.len())
            .map(|_| 2.0 + (xorshift64(&mut st.gen) % 3000) as f64 / 1000.0)
            .collect();
        for (d, gap) in st.domains.iter_mut().zip(gaps) {
            d.checking_until = Some(t + gap);
        }
        Ok(count)
    }

    fn add_domain(&self, domain: &str) -> Result<String, ActionError> {
        let name = domain.trim().to_ascii_lowercase();
        let valid = !name.is_empty()
            && name.len() <= 253
            && name.contains('.')
            && !name.starts_with('.')
            && !name.ends_with('.')
            && !name.starts_with('-')
            && !name.ends_with('-')
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
        if !valid {
            return Err(ActionError::Invalid(
                "couldn't parse the domain name".to_string(),
            ));
        }
        let mut st = self.state.lock().unwrap();
        if !st.domains.iter().any(|d| d.name == name) {
            st.domains.push(DemoDomain {
                name: name.clone(),
                checking_until: None,
                last_verdict: None,
            });
        }
        Ok(name)
    }

    fn remove_domain(&self, domain: &str) -> Result<String, ActionError> {
        let name = domain.trim().to_ascii_lowercase();
        let mut st = self.state.lock().unwrap();
        st.domains.retain(|d| d.name != name);
        Ok(name)
    }

    fn add_peer_address(&self, line: &str) -> Result<(), String> {
        let (id_part, _rest) = line
            .split_once('@')
            .ok_or_else(|| "no @ separator".to_string())?;
        if id_part.is_empty() {
            return Err("no identifier before @".to_string());
        }
        let t = self.state.lock().unwrap().t;
        self.push_log(t, "member", format!("peer address added: {line}"));
        Ok(())
    }

    fn apply_config(&self, settings: &Settings, paths: &[&'static str]) {
        let (t, ring_lines) = {
            let mut st = self.state.lock().unwrap();
            st.settings = settings.clone();
            (st.t, settings.process.log_ring_lines)
        };
        self.push_log(t, "node", format!("settings applied: {}", paths.join(", ")));
        if paths.contains(&"log.ring_lines") {
            self.log
                .lock()
                .unwrap()
                .set_capacity((ring_lines as usize).max(1));
        }
    }

    fn switch_network(&self, key: &str) {
        let (t, label) = {
            let mut st = self.state.lock().unwrap();
            st.boot_id = gen_hex8(&mut st.gen);
            st.status = NodeStatus::Starting;
            st.starting_until = Some(st.t + 3.0);
            let label = key.split("--").next().unwrap_or(key).to_string();
            (st.t, label)
        };
        self.log.lock().unwrap().clear();
        self.push_log(t, "node", format!("switching to network \"{label}\""));
    }

    fn restart(&self) -> Result<(), ActionError> {
        let t = {
            let mut st = self.state.lock().unwrap();
            if st.status == NodeStatus::Stopping {
                return Err(ActionError::Busy(format!(
                    "{} is stopping",
                    meshtown_core::node::output::NET_NAME
                )));
            }
            st.boot_id = gen_hex8(&mut st.gen);
            st.status = NodeStatus::Starting;
            st.starting_until = Some(st.t + 3.0);
            st.t
        };
        self.push_log(t, "node", "node restart requested by the page".to_string());
        Ok(())
    }

    fn invite(&self) -> Result<String, ActionError> {
        invite_for(&key_of_node(self))
    }

    fn join_by_invite(
        &self,
        head: InviteHead,
        _peers: Vec<PeerAddr>,
        save: SaveKey,
    ) -> Result<(), ActionError> {
        enter(self, &head, save)
    }

    fn stop(&self) {
        {
            let mut st = self.state.lock().unwrap();
            st.status = NodeStatus::Stopping;
        }
        self.notify.notify_one();
    }
}
