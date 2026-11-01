//! Three `join` commands with one key run at once against one node: the network is joined once, the others
//! say "already".

#[path = "common/cli_node.rs"]
mod cli_node;
#[path = "common/join_fixtures.rs"]
mod fixtures;

use cli_node::{cli_in, stderr, stdout, CliNode};
use fixtures::key_with_label;
use serde_json::Value;
use std::sync::Barrier;

const ROUNDS: usize = 6;
const CLIENTS: usize = 3;

/// Runs one round on a fresh node; returns what went wrong, with every client's exit code and stderr.
fn one_round(round: usize) -> Result<(), String> {
    let node = CliNode::start(&[]);
    let key = key_with_label("room-par", 5);
    let barrier = Barrier::new(CLIENTS);
    let outs: Vec<(Option<i32>, String)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..CLIENTS)
            .map(|_| {
                let (dir, port, key, barrier) = (&node.dir, node.port, &key, &barrier);
                scope.spawn(move || {
                    barrier.wait();
                    let out = cli_in(&["join", key], dir, port);
                    (out.status.code(), stderr(&out).trim().replace('\n', " | "))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("client"))
            .collect()
    });
    let report = format!("round {round}: {outs:?}");
    println!("{report}");
    let joined = outs.iter().filter(|o| o.1.contains("Joined")).count();
    let already = outs
        .iter()
        .filter(|o| o.1.contains("already in this network"))
        .count();
    let status = node.cli(&["status", "--json"]);
    let reply: Value = serde_json::from_str(stdout(&status).trim()).unwrap_or(Value::Null);
    let nets = reply["nets"].as_array().map_or(0, Vec::len);
    let ok = outs.iter().all(|o| o.0 == Some(0)) && joined == 1 && already == 2 && nets == 1;
    if ok {
        return Ok(());
    }
    Err(format!(
        "{report}; joined={joined} already={already} nets={nets}"
    ))
}

#[test]
fn three_parallel_joins_of_one_key_join_once() {
    let failures: Vec<String> = (0..ROUNDS).filter_map(|r| one_round(r).err()).collect();
    assert!(
        failures.is_empty(),
        "{} of {ROUNDS} rounds failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
