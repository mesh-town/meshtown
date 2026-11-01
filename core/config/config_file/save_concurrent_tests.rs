use super::*;
use crate::config::config_file::settings::Settings;
use crate::config::config_file::test_support::{scratch, test_secret};
use crate::config::config_file::{load_config_file, LoadedConfigFile, FILE_NAME};
use std::sync::Barrier;

const ROUNDS: usize = 40;
const WRITERS: usize = 8;

/// One round: `WRITERS` threads save over the same revision; returns a description of what went wrong.
fn race_once(round: usize) -> Vec<String> {
    let dir = scratch("race");
    let file = dir.join(FILE_NAME);
    let first = save_config_file(&file, None, 1, &test_secret(), &Settings::default());
    assert_eq!(first.expect("initial save"), 1);
    let barrier = Barrier::new(WRITERS);
    let results: Vec<Result<u64, SaveError>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..WRITERS)
            .map(|n| {
                let (file, barrier) = (&file, &barrier);
                scope.spawn(move || {
                    let mut s = Settings::default();
                    s.net.node_name = Some(format!("writer-{n}"));
                    barrier.wait();
                    save_config_file(file, Some(1), 2, &test_secret(), &s)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("writer"))
            .collect()
    });
    let mut problems = Vec::new();
    let oks = results.iter().filter(|r| r.is_ok()).count();
    if oks != 1 {
        problems.push(format!("round {round}: {oks} writers got Ok"));
    }
    for r in &results {
        match r {
            Err(SaveError::Io(m)) => problems.push(format!("round {round}: Io: {m}")),
            Ok(rev) if *rev != 2 => problems.push(format!("round {round}: Ok({rev})")),
            _ => {}
        }
    }
    match load_config_file(&file) {
        LoadedConfigFile::Parsed(p) if p.rev == 2 => {}
        LoadedConfigFile::Parsed(p) => problems.push(format!("round {round}: file rev {}", p.rev)),
        other => problems.push(format!("round {round}: unreadable file: {other:?}")),
    }
    let leftovers: Vec<String> = std::fs::read_dir(&dir)
        .expect("read dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("config.json.tmp-"))
        .collect();
    if !leftovers.is_empty() {
        problems.push(format!("round {round}: leftover {leftovers:?}"));
    }
    let _ = std::fs::remove_dir_all(&dir);
    problems
}

#[test]
fn racing_writers_with_one_rev_succeed_once() {
    let problems: Vec<String> = (0..ROUNDS).flat_map(race_once).collect();
    assert!(
        problems.is_empty(),
        "{} problems in {ROUNDS} rounds; first: {}\nall: {problems:#?}",
        problems.len(),
        problems[0]
    );
}
