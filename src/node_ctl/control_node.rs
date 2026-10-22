//! Page actions on the node: snapshot, adding a member address, applying settings.
//! Bodies of the `NodeControl for NodeCtl` methods; the trait block in `control.rs` only delegates here.

use std::time::Duration;

use tokio::time::Instant;

use crate::config_file::Settings;
use crate::output::{self, say};
use crate::web::NodeSnapshot;

use super::live_paths::{apply_log_capacity, apply_pending_handles, copy_live_paths};
use super::peer_addr::parse_peer_line;
use super::snapshot;
use super::state::{NodeCtl, Phase};

impl NodeCtl {
    /// `boot_id` of the slot's current lap.
    pub(super) fn boot_id(&self) -> String {
        let st = self.ctl_shared.ctl_state.lock().unwrap_or_else(|e| e.into_inner());
        st.boot.id().to_string()
    }

    pub(super) fn snapshot_impl(&self) -> NodeSnapshot {
        let (status, mut snap, node, was_stale, boot_id, uptime) = {
            let mut st = self.ctl_shared.ctl_state.lock().unwrap_or_else(|e| e.into_inner());
            let status = snapshot::status_of(&st.phase, st.cache.as_ref());
            let snap = st
                .cache
                .as_ref()
                .map(|c| c.snap.clone())
                .unwrap_or_default();
            let node = st.node.clone();
            let stale = st.last_read.elapsed() > Duration::from_secs(5);
            st.last_read = Instant::now();
            (status, snap, node, stale, st.boot.id().to_string(), st.boot.uptime_sec())
        };
        if was_stale {
            self.ctl_shared.kick.notify_one();
        }
        snap.node.status = status;
        if let Some(n) = &node {
            snap.node.id = output::short_peer_id(&n.id);
            snap.node.id_full = n.id.to_string();
            snapshot::overlay_members(&mut snap, n);
        }
        snap.boot_id = boot_id;
        snap.node.uptime_sec = uptime;
        snapshot::overlay_process(&mut snap, snapshot::ProcessFacts::collect());
        snap
    }

    pub(super) fn add_peer_address_impl(&self, line: &str) -> Result<(), String> {
        let node = {
            let st = self.ctl_shared.ctl_state.lock().unwrap_or_else(|e| e.into_inner());
            match st.phase {
                Phase::Stopping => return Err("node is stopping".to_string()),
                _ => match &st.node {
                    Some(n) => n.clone(),
                    None => return Err("node is not started yet".to_string()),
                },
            }
        };
        let addr = parse_peer_line(line)?;
        if node.id == addr.id {
            return Err("this is the node's own address".to_string());
        }
        let line_owned = line.trim().to_string();
        self.rt.spawn(async move {
            node.introduce(addr).await;
            say("member", format!("member address added: {line_owned}"));
        });
        Ok(())
    }

    pub(super) fn apply_config_impl(&self, settings: &Settings, paths: &[&'static str]) {
        let (live, deferred, node, effective_settings) = {
            let mut st = self.ctl_shared.ctl_state.lock().unwrap_or_else(|e| e.into_inner());
            let (live, deferred) = copy_live_paths(&mut st.effective_settings, settings, paths);
            if st.node.is_none() {
                for p in &live {
                    if !st.pending.contains(p) {
                        st.pending.push(p);
                    }
                }
            }
            (live, deferred, st.node.clone(), st.effective_settings.clone())
        };
        if live.contains(&"log.ring_lines") {
            apply_log_capacity(&effective_settings);
        }
        if let Some(n) = &node {
            apply_pending_handles(n, &live, &effective_settings);
        }
        let msg = match (live.is_empty(), deferred.is_empty()) {
            (true, true) => "settings applied: no changes".to_string(),
            (false, true) => format!("settings applied: {}", live.join(", ")),
            (true, false) => format!(
                "settings applied: take effect after a restart: {}",
                deferred.join(", ")
            ),
            (false, false) => format!(
                "settings applied: {}; take effect after a restart: {}",
                live.join(", "),
                deferred.join(", ")
            ),
        };
        say("node", msg);
        self.ctl_shared.kick.notify_one();
    }
}
