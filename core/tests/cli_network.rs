//! The `network` group of the real binary: `list`, `create`, `key`, `inspect` and `delete` against a running node
//! (a real `run` process), and the commands that need no node.

#[path = "common/cli_node.rs"]
mod cli_node;

use std::path::PathBuf;

use meshtown_core::members::network_key::parse_user_input;

use cli_node::{cli_in as mesh, free_port, scratch_dir, stderr as err, stdout as out, CliNode};

/// A fresh directory only its owner can enter, so the commands have nothing to warn about.
fn private_dir(tag: &str) -> PathBuf {
    let dir = scratch_dir(tag);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("mode");
    }
    dir
}

fn code(o: &std::process::Output) -> Option<i32> {
    o.status.code()
}

mod without_a_node {
    use super::*;

    fn nothing_listens(args: &[&str]) {
        let dir = private_dir("net-nonode");
        let o = mesh(args, &dir, free_port());
        assert_eq!(code(&o), Some(3), "{args:?} stderr: {}", err(&o));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_create_and_inspect_need_a_node() {
        nothing_listens(&["network", "list"]);
        nothing_listens(&["network", "create", "ourgame"]);
        nothing_listens(&["network", "inspect", "x"]);
    }

    #[test]
    fn key_only_raw_prints_one_machine_key() {
        let dir = private_dir("net-keyonly");
        let o = mesh(
            &["network", "create", "--key-only", "--raw"],
            &dir,
            free_port(),
        );
        assert_eq!(code(&o), Some(0), "stderr: {}", err(&o));
        let text = out(&o);
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.starts_with("mesh2-"), "{text}");
        let parsed = parse_user_input(text.trim()).expect("parses");
        assert!(dir.join("config.json").exists());
        assert_eq!(parsed.label, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn key_only_refuses_json() {
        let dir = private_dir("net-keyonly-json");
        let o = mesh(
            &["network", "create", "--key-only", "--json"],
            &dir,
            free_port(),
        );
        assert_eq!(code(&o), Some(2), "stderr: {}", err(&o));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_without_a_label_is_a_usage_error() {
        let dir = private_dir("net-nolabel");
        let o = mesh(&["network", "create"], &dir, free_port());
        assert_eq!(code(&o), Some(2), "stderr: {}", err(&o));
        assert!(err(&o).contains("network create <label>"), "{}", err(&o));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_label_the_node_would_refuse_is_a_usage_error() {
        let dir = private_dir("net-badlabel");
        let o = mesh(&["network", "create", "!!!"], &dir, free_port());
        assert_eq!(code(&o), Some(2), "stderr: {}", err(&o));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_without_yes_is_a_usage_error() {
        let dir = private_dir("net-noyes");
        let o = mesh(&["network", "delete"], &dir, free_port());
        assert_eq!(code(&o), Some(2), "stderr: {}", err(&o));
        assert!(err(&o).contains("--yes"), "{}", err(&o));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn keygen_is_replaced_by_network_create_key_only() {
        let dir = private_dir("net-keygen");
        let o = mesh(&["keygen"], &dir, free_port());
        assert_eq!(code(&o), Some(2), "stderr: {}", err(&o));
        assert!(err(&o).contains("network create --key-only"), "{}", err(&o));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

mod with_a_node {
    use super::*;

    fn lines(o: &std::process::Output) -> Vec<String> {
        out(o).lines().map(str::to_string).collect()
    }

    /// Runs a command again while the node answers that it is starting: creating a network restarts it.
    fn settled(node: &CliNode, args: &[&str]) -> std::process::Output {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            let o = node.cli(args);
            let starting = err(&o).contains("is starting");
            if !starting || std::time::Instant::now() > deadline {
                return o;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    #[test]
    fn create_list_key_inspect_and_delete() {
        // given: a node with no network
        let node = CliNode::start(&[]);
        // when: a network is created
        let made = node.cli(&["network", "create", "ourgame"]);
        assert_eq!(code(&made), Some(0), "stderr: {}", err(&made));
        let key = out(&made).trim().to_string();
        // then: stdout is a key that parses, and `key` prints the same one
        assert_eq!(
            parse_user_input(&key).expect("parses").label.as_deref(),
            Some("ourgame")
        );
        let shown = settled(&node, &["network", "key"]);
        assert_eq!(out(&shown).trim(), key);
        let raw = settled(&node, &["network", "key", "--raw"]);
        assert!(out(&raw).starts_with("ourgame--mesh2-"), "{}", out(&raw));
        let listed = settled(&node, &["network", "list"]);
        let rows = lines(&listed);
        assert!(rows[0].starts_with("SLOT"), "{rows:?}");
        assert!(
            rows[1].starts_with("main") && rows[1].contains("ourgame"),
            "{rows:?}"
        );
        // when: the key is inspected
        let inspected = settled(&node, &["network", "inspect", &key]);
        let text = out(&inspected);
        assert!(
            text.contains("kind: key") && text.contains("label: ourgame"),
            "{text}"
        );
        // when: a second network is created, it goes to a new slot
        let other = settled(&node, &["network", "create", "other"]);
        assert_eq!(code(&other), Some(0), "stderr: {}", err(&other));
        assert!(
            err(&other).contains("Created in a new slot"),
            "{}",
            err(&other)
        );
        assert_eq!(lines(&settled(&node, &["network", "list"])).len(), 3);
        let json = settled(&node, &["network", "list", "--json"]);
        assert!(out(&json).contains("\"nets\""), "{}", out(&json));
        // when: the second one is deleted by its label
        let gone = settled(&node, &["network", "delete", "--yes", "--net", "other"]);
        assert_eq!(code(&gone), Some(0), "stderr: {}", err(&gone));
        assert!(
            err(&gone).contains("Network deleted for everyone"),
            "{}",
            err(&gone)
        );
        assert_eq!(lines(&settled(&node, &["network", "list"])).len(), 2);
        // then: the same delete again finds no such network
        let again = settled(&node, &["network", "delete", "--yes", "--net", "other"]);
        assert_eq!(code(&again), Some(1), "stderr: {}", err(&again));
    }
}
