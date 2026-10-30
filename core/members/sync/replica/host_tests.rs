use super::*;

mod bak_policy {
    use super::*;

    #[test]
    fn mirror_ignores_main() {
        assert!(matches!(
            super::super::bak_policy(true, false),
            BakPolicy::Mirror
        ));
    }

    #[test]
    fn first_save_keeps_main_ok() {
        for flag in [true, false] {
            match super::super::bak_policy(false, flag) {
                BakPolicy::FirstSave { main_ok } => assert_eq!(main_ok, flag),
                _ => panic!("expected FirstSave"),
            }
        }
    }
}

mod unpack_loaded {
    use super::*;

    fn run(loaded: Loaded) -> (Option<ImportMark>, Vec<WireEntry>, bool) {
        super::super::unpack_loaded(loaded, "net", Path::new("net.bin"))
    }

    #[test]
    fn fresh_is_empty() {
        let (mark, entries, ok) = run(Loaded::Fresh);
        assert!(mark.is_none() && entries.is_empty() && !ok);
    }

    #[test]
    fn main_is_ok() {
        assert!(run(Loaded::Main(None, vec![])).2);
    }

    #[test]
    fn backup_is_not_ok() {
        assert!(!run(Loaded::Backup(None, vec![])).2);
    }
}
