//! Page actions on the node: snapshot, adding a member address, applying settings.
//! Bodies of the `NodeControl for NodeCtl` methods; the trait block in `control.rs` only delegates here.

use std::time::Duration;

use tokio::time::Instant;

use crate::config::config_file::Settings;
use crate::node::output::{self, say};
use crate::web_gui::NodeSnapshot;

use super::live_paths::{apply_log_capacity, apply_pending_handles, copy_live_paths};
use super::peer_addr::parse_peer_line;
use super::snapshot;
use super::state::{NodeCtl, Phase};

impl NodeCtl {
    /// `boot_id` of the slot's current lap.
    pub(super) fn boot_id(&self) -> String {
        let st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        st.boot.id().to_string()
    }

    pub(super) fn snapshot_impl(&self) -> NodeSnapshot {
        let (fixed, mut snap, node, was_stale, boot_id, uptime) = {
            let mut st = self
                .ctl_shared
                .ctl_state
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let fixed = snapshot::phase_status(&st.phase, st.cache.is_some());
            let snap = st
                .cache
                .as_ref()
                .map(|c| c.snap.clone())
                .unwrap_or_default();
            let node = st.node.clone();
            let stale = st.last_read.elapsed() > Duration::from_secs(5);
            st.last_read = Instant::now();
            (
                fixed,
                snap,
                node,
                stale,
                st.boot.id().to_string(),
                st.boot.uptime_sec(),
            )
        };
        if was_stale {
            self.ctl_shared.kick.notify_one();
        }
        if let Some(n) = &node {
            snap.node.id = output::short_peer_id(&n.id);
            snap.node.id_full = n.id.to_string();
            snapshot::overlay_members(&mut snap, n);
        } else if let Some(face) = self.proxy_face() {
            snap.socks = snapshot::socks_of_face(&face.borrow());
        }
        snap.node.status = fixed.unwrap_or_else(|| snapshot::link_status(&snap));
        self.overlay_web_urls(&mut snap);
        snap.boot_id = boot_id;
        snap.node.uptime_sec = uptime;
        snapshot::overlay_process(&mut snap, snapshot::ProcessFacts::collect());
        snap
    }

    /// Fills `web_url` of the shared ports from the mesh addresses the process proxy answers.
    fn overlay_web_urls(&self, snap: &mut NodeSnapshot) {
        let Some(front) = self.proxy_front() else {
            return;
        };
        snapshot::overlay_web_urls(&mut snap.ports.network, &front.mesh_addresses());
    }

    pub(super) fn add_peer_address_impl(&self, line: &str) -> Result<(), String> {
        let node = {
            let st = self
                .ctl_shared
                .ctl_state
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match st.phase {
                Phase::Stopping => return Err(super::gates::stopping_text()),
                _ => match &st.node {
                    Some(n) => n.clone(),
                    None => {
                        return Err(format!(
                            "{} has not started yet",
                            crate::node::output::NET_NAME
                        ))
                    }
                },
            }
        };
        let addr = parse_peer_line(line)?;
        if node.id == addr.id {
            return Err("this address belongs to this computer".to_string());
        }
        let line_owned = line.trim().to_string();
        crate::node::output::spawn_on(&self.rt, async move {
            node.introduce(addr).await;
            say("member", format!("member address added: {line_owned}"));
        });
        Ok(())
    }

    pub(super) fn apply_config_impl(&self, settings: &Settings, paths: &[&'static str]) {
        let (live, deferred, node, effective_settings) = {
            let mut st = self
                .ctl_shared
                .ctl_state
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let (live, deferred) = copy_live_paths(&mut st.effective_settings, settings, paths);
            if st.node.is_none() {
                for p in &live {
                    if !st.pending.contains(p) {
                        st.pending.push(p);
                    }
                }
            }
            (
                live,
                deferred,
                st.node.clone(),
                st.effective_settings.clone(),
            )
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

#[cfg(test)]
#[path = "control_node_status_tests.rs"]
mod status_tests;
