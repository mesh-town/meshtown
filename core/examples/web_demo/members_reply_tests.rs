use clap::Parser;
use meshtown_core::config::config_file::Settings;
use serde_json::Value;

use crate::args::Args;
use crate::node::{DemoNetworkOptions, DemoNode};

fn reply() -> Value {
    let args = Args::parse_from(["web_demo", "--members-pending"]);
    let opts = DemoNetworkOptions {
        offline_peers: 0,
        no_exit_peers: 0,
        admission_refused: false,
        dht_joined: false,
        dht_nodes: 0,
        members_pending: true,
        me_excluded: false,
        managed: Default::default(),
    };
    let node = DemoNode::new(1, args.fast, 100, false, opts, Settings::default());
    serde_json::to_value(node.members_list().expect("a list")).expect("json")
}

mod reply_shape {
    use super::*;

    #[test]
    fn the_top_level_has_no_vote_fields() {
        let r = reply();
        for key in ["kind", "need", "me_removed"] {
            assert!(r.get(key).is_none(), "unexpected `{key}`");
        }
    }

    #[test]
    fn the_counts_have_no_vote_fields() {
        let counts = &reply()["counts"];
        assert!(counts.get("voting").is_none());
        assert!(counts.get("forgotten").is_none());
    }

    #[test]
    fn a_row_has_no_vote_or_forget() {
        for row in reply()["members"].as_array().expect("rows") {
            assert!(row.get("vote").is_none(), "{row}");
            assert!(row.get("forget").is_none(), "{row}");
        }
    }

    #[test]
    fn the_creator_is_the_root_and_an_admin() {
        let r = reply();
        assert_eq!(r["root"], r["me"]);
        assert_eq!(r["me_admin"], true);
    }

    #[test]
    fn a_pending_row_carries_only_the_invite_note() {
        let r = reply();
        let rows = r["members"].as_array().expect("rows");
        let pending = rows
            .iter()
            .find(|m| m["status"] == "pending")
            .expect("a pending row");
        assert_eq!(pending["admission"]["label"], "Anna's laptop");
        assert!(pending["admission"].get("votes").is_none());
    }
}

mod exclusion_reason {
    use meshtown_core::transport::net::NodeSecret;
    use meshtown_core::web_gui::MemberActionReq as Act;

    use crate::managed::DemoRole;
    use crate::members::DemoMembers;
    use crate::members_reply::build_reply;
    use crate::node::{DemoNetworkOptions, DemoPeer};

    fn demo(role: DemoRole, me_excluded: bool) -> DemoMembers {
        let opts = DemoNetworkOptions {
            offline_peers: 0,
            no_exit_peers: 0,
            admission_refused: false,
            dht_joined: false,
            dht_nodes: 0,
            me_excluded,
            members_pending: false,
            managed: role,
        };
        let id = NodeSecret::from_bytes(&[2; 32]).peer_id();
        let peer = DemoPeer {
            id: id.to_string()[..8].to_string(),
            id_full: id,
            name: Some("peer-one".to_string()),
            path: None,
            online: true,
            exit: true,
        };
        DemoMembers::new(&[peer], &opts)
    }

    fn id_of(m: &DemoMembers, name: &str) -> String {
        let e = m.entries.iter().find(|e| e.name.as_deref() == Some(name));
        e.unwrap().id_full.clone()
    }

    /// The `reason` slot of the row named `name`: outer `None` — no key.
    fn reason_of(m: &DemoMembers, name: &str) -> Option<Option<String>> {
        let reply = build_reply(m, "abcd1234", None);
        let row = reply
            .members
            .iter()
            .find(|r| r.name.as_deref() == Some(name));
        row.unwrap()
            .excluded
            .as_ref()
            .expect("excluded")
            .reason
            .clone()
    }

    fn exclude(m: &mut DemoMembers, reason: Option<&str>) {
        let id = id_of(m, "peer-one");
        let reason = reason.map(str::to_string);
        m.apply(&Act::Exclude { id, reason }).expect("excluded");
    }

    #[test]
    fn exclude_with_reason_shows_it_in_the_reply() {
        let mut m = demo(DemoRole::Root, false);
        exclude(&mut m, Some("spam"));
        assert_eq!(reason_of(&m, "peer-one"), Some(Some("spam".to_string())));
        let json = serde_json::to_value(build_reply(&m, "abcd1234", None)).expect("json");
        let row = json["members"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == "peer-one");
        assert_eq!(row.unwrap()["excluded"]["reason"], "spam");
    }

    #[test]
    fn exclude_without_reason_is_null() {
        let mut m = demo(DemoRole::Root, false);
        exclude(&mut m, None);
        assert_eq!(reason_of(&m, "peer-one"), Some(None));
        let json = serde_json::to_value(build_reply(&m, "abcd1234", None)).expect("json");
        let row = json["members"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == "peer-one");
        assert!(row.unwrap()["excluded"]["reason"].is_null());
        assert!(row.unwrap()["excluded"]
            .as_object()
            .unwrap()
            .contains_key("reason"));
    }

    #[test]
    fn a_non_admin_gets_no_key_for_other_rows() {
        let m = demo(DemoRole::Member, false);
        assert_eq!(reason_of(&m, "peer-three"), None);
    }

    #[test]
    fn the_own_row_carries_the_reason_even_for_a_non_admin() {
        let m = demo(DemoRole::Member, true);
        let reply = build_reply(&m, "abcd1234", None);
        let me = reply.members.iter().find(|r| r.is_self).unwrap();
        let reason = me.excluded.as_ref().unwrap().reason.clone();
        assert_eq!(reason, Some(Some("Out of the project".to_string())));
    }
}
