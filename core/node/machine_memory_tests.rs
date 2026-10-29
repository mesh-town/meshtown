//! Tests of the machine-memory tiers, the warning-level defaults and the start-up line.

use super::*;

const MIB: u64 = 1024;

mod defaults {
    use super::*;

    #[test]
    fn tiers_hold_the_agreed_numbers() {
        assert_eq!(
            (
                MemoryDefaults::SMALL.max_streams,
                MemoryDefaults::SMALL.warn_mib,
                MemoryDefaults::SMALL.rss_guard,
                MemoryDefaults::SMALL.stream_window_kib
            ),
            (20, 24, true, 128)
        );
        assert_eq!(
            (
                MemoryDefaults::MIDDLE.max_streams,
                MemoryDefaults::MIDDLE.warn_mib,
                MemoryDefaults::MIDDLE.rss_guard,
                MemoryDefaults::MIDDLE.stream_window_kib
            ),
            (50, 64, true, 0)
        );
        assert_eq!(
            (
                MemoryDefaults::LARGE.max_streams,
                MemoryDefaults::LARGE.warn_mib,
                MemoryDefaults::LARGE.rss_guard,
                MemoryDefaults::LARGE.stream_window_kib
            ),
            (200, 400, false, 0)
        );
    }

    #[test]
    fn small_machine_gets_small_defaults() {
        for kib in [64 * MIB, 131_072] {
            assert_eq!(for_ram(Some(kib)), MemoryDefaults::SMALL, "{kib} KiB");
        }
    }

    #[test]
    fn middle_machine_gets_middle_defaults() {
        for kib in [131_073, 132_096, 497 * MIB, 524_288] {
            assert_eq!(for_ram(Some(kib)), MemoryDefaults::MIDDLE, "{kib} KiB");
        }
    }

    #[test]
    fn larger_machine_gets_pc_defaults() {
        for kib in [525_312, 16 * 1024 * MIB] {
            assert_eq!(for_ram(Some(kib)), MemoryDefaults::LARGE, "{kib} KiB");
        }
    }

    #[test]
    fn unknown_ram_gets_pc_defaults() {
        assert_eq!(for_ram(None), MemoryDefaults::LARGE);
    }
}

mod warn {
    use super::*;

    #[test]
    fn default_400_follows_the_machine() {
        let cases = [
            (MemoryDefaults::SMALL, 24),
            (MemoryDefaults::MIDDLE, 64),
            (MemoryDefaults::LARGE, 400),
            (for_ram(None), 400),
        ];
        for (tier, expected) in cases {
            assert_eq!(effective_warn_mib(400, tier), expected, "{tier:?}");
        }
    }

    #[test]
    fn explicit_value_wins() {
        for tier in [
            MemoryDefaults::SMALL,
            MemoryDefaults::MIDDLE,
            MemoryDefaults::LARGE,
        ] {
            assert_eq!(effective_warn_mib(100, tier), 100, "{tier:?}");
            assert_eq!(effective_warn_mib(0, tier), 0, "{tier:?}");
        }
    }
}

mod guard {
    use super::*;

    #[test]
    fn pc_default_turns_the_rss_guard_off() {
        assert_eq!(rss_guard_warn_mib(400, MemoryDefaults::LARGE), 0);
    }

    #[test]
    fn small_and_middle_machines_keep_the_guard() {
        assert_eq!(rss_guard_warn_mib(400, MemoryDefaults::SMALL), 24);
        assert_eq!(rss_guard_warn_mib(400, MemoryDefaults::MIDDLE), 64);
    }

    #[test]
    fn explicit_warn_keeps_the_rss_guard_on_a_pc() {
        assert_eq!(rss_guard_warn_mib(300, MemoryDefaults::LARGE), 300);
        for tier in [
            MemoryDefaults::SMALL,
            MemoryDefaults::MIDDLE,
            MemoryDefaults::LARGE,
        ] {
            assert_eq!(
                rss_guard_warn_mib(0, tier),
                0,
                "explicit 0 turns the guard off: {tier:?}"
            );
        }
    }
}

mod meminfo {
    use super::*;

    #[test]
    fn reads_the_total_in_kib() {
        let text = "MemFree:         1000 kB\nMemTotal:        4036544 kB\nBuffers: 5 kB\n";
        assert_eq!(mem_total_kib(text), Some(4_036_544));
    }

    #[test]
    fn missing_total_gives_none() {
        assert_eq!(mem_total_kib("MemFree: 1000 kB\nBuffers: 5 kB\n"), None);
    }

    #[test]
    fn garbage_number_gives_none() {
        assert_eq!(mem_total_kib("MemTotal:   lots kB\n"), None);
    }
}

mod line {
    use super::*;

    #[test]
    fn names_ram_streams_and_warn() {
        let known = memory_line(Some(497 * MIB), 50, 64);
        assert!(known.contains("machine 497 MiB"), "got: {known}");
        assert!(known.contains("exit streams 50"), "got: {known}");
        assert!(known.contains("warn 64 MiB"), "got: {known}");

        let unknown = memory_line(None, 200, 400);
        assert!(unknown.contains("unknown"), "got: {unknown}");
        assert!(unknown.contains("exit streams 200"), "got: {unknown}");
        assert!(unknown.contains("warn 400 MiB"), "got: {unknown}");
    }
}
