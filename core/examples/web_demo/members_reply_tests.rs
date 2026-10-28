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
