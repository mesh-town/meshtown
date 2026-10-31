//! The `member` group of the real binary: the list and the refusals of a running node, and the command
//! line errors that need no node.

#[path = "common/cli_node.rs"]
mod cli_node;

use std::path::PathBuf;
use std::process::Output;
use std::time::{Duration, Instant};

use cli_node::{cli_in as mesh, free_port, scratch_dir, stderr as err, stdout as out, CliNode};

fn private_dir(tag: &str) -> PathBuf {
    let dir = scratch_dir(tag);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("mode");
    }
    dir
}

fn code(o: &Output) -> Option<i32> {
    o.status.code()
}

mod without_a_node {
    use super::*;

    fn run(tag: &str, args: &[&str]) -> Output {
        let dir = private_dir(tag);
        let o = mesh(args, &dir, free_port());
        let _ = std::fs::remove_dir_all(&dir);
        o
    }

    #[test]
    fn no_node_is_three() {
        let o = run("mem-nonode", &["member", "list"]);
        assert_eq!(code(&o), Some(3), "stderr: {}", err(&o));
    }

    #[test]
    fn bad_reason_is_usage_before_connect() {
        let long = "x".repeat(81);
        let o = run(
            "mem-reason",
            &["member", "exclude", "laptop", "--reason", &long],
        );
        assert_eq!(code(&o), Some(2), "stderr: {}", err(&o));
        assert!(err(&o).contains("--reason"), "{}", err(&o));
    }

    #[test]
    fn yes_flag_is_usage() {
        let o = run("mem-yes", &["member", "list", "--yes"]);
        assert_eq!(code(&o), Some(2), "stderr: {}", err(&o));
        assert!(err(&o).contains("--yes"), "{}", err(&o));
    }

    #[test]
    fn missing_member_arg_is_usage() {
        let o = run("mem-noarg", &["member", "admit"]);
        assert_eq!(code(&o), Some(2), "stderr: {}", err(&o));
    }

    #[test]
    fn member_help_lists_all_verbs() {
        let o = run("mem-help", &["member", "--help"]);
        assert_eq!(code(&o), Some(0), "stderr: {}", err(&o));
        let text = out(&o);
        for verb in [
            "list",
            "admit",
            "exclude",
            "unexclude",
            "block",
            "unblock",
            "admin",
        ] {
            assert!(text.contains(verb), "{verb} missing in {text}");
        }
    }
}

mod with_a_node {
    use super::*;

    /// Runs a command again while the node answers that it is starting: creating a network restarts it.
    fn settled(node: &CliNode, args: &[&str]) -> Output {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let o = node.cli(args);
            let waiting = err(&o).contains("is starting") || err(&o).contains("no network");
            if !waiting || Instant::now() > deadline {
                return o;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn node_with_network() -> CliNode {
        let node = CliNode::start(&[]);
        let made = node.cli(&["network", "create", "ourgame"]);
        assert_eq!(code(&made), Some(0), "stderr: {}", err(&made));
        node
    }

    fn own_id(node: &CliNode) -> String {
        let listed = settled(node, &["member", "list"]);
        let text = out(&listed);
        let row = text
            .lines()
            .nth(1)
            .unwrap_or_else(|| panic!("no row: {text}"));
        row.split_whitespace().next().expect("id").to_string()
    }

    #[test]
    fn list_shows_self_row() {
        let node = node_with_network();
        let o = settled(&node, &["member", "list"]);
        assert_eq!(code(&o), Some(0), "stderr: {}", err(&o));
        let text = out(&o);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].starts_with("ID"), "{text}");
        assert!(lines[1].contains("(this computer)"), "{text}");
        let json = settled(&node, &["member", "list", "--json"]);
        assert!(out(&json).contains("\"members\""), "{}", out(&json));
    }

    #[test]
    fn admit_self_refused() {
        let node = node_with_network();
        let id = own_id(&node);
        let o = settled(&node, &["member", "admit", &id]);
        assert_eq!(code(&o), Some(1), "stderr: {}", err(&o));
        assert!(err(&o).contains("your own computer"), "{}", err(&o));
    }

    /// The creator of a network keeps the role, so the node refuses to take it away.
    #[test]
    fn revoke_creator_refused() {
        let node = node_with_network();
        let id = own_id(&node);
        let o = settled(&node, &["member", "admin", &id, "--revoke"]);
        assert_eq!(code(&o), Some(1), "stderr: {}", err(&o));
        assert!(err(&o).contains("only admins can do this"), "{}", err(&o));
        assert!(err(&o).contains("Ask an administrator"), "{}", err(&o));
    }

    #[test]
    fn unknown_net_lists_nets() {
        let node = node_with_network();
        let o = settled(&node, &["member", "list", "--net", "nope"]);
        assert_eq!(code(&o), Some(1), "stderr: {}", err(&o));
        assert!(err(&o).contains("no network matches"), "{}", err(&o));
    }
}
