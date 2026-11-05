//! Tests of `aside.rs`: the files that leave with the main network.

use super::set_aside_main;
use crate::config::config_file::test_support::{names_in, sample_settings, scratch, test_secret};
use crate::config::config_file::{render_config_file, FILE_NAME};

const NOW: u64 = 1_790_278_867;

#[test]
fn set_aside_main_moves_every_peers_cache_file() {
    // given: the old cache, a per-network cache and the names file beside the main file
    let root = scratch("aside-main");
    let main = root.join(FILE_NAME);
    let text = render_config_file(1, NOW, &test_secret(), &sample_settings());
    std::fs::write(&main, text).expect("main file");
    for file in [
        "peers-cache.json",
        "peers-cache-0123456789abcdef.json",
        "peer-names.json",
    ] {
        std::fs::write(root.join(file), "{}").expect("cache file");
    }
    // when: the main network is set aside
    let target = set_aside_main(&main, NOW).expect("set aside");
    // then: all three left with it and none stays beside the main file
    assert_eq!(
        names_in(&target),
        [
            "config.json",
            "peer-names.json",
            "peers-cache-0123456789abcdef.json",
            "peers-cache.json"
        ]
    );
    assert_eq!(names_in(&root), ["config.json", "nets"]);
    let _ = std::fs::remove_dir_all(&root);
}
