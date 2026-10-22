//! Tests for `tickets.rs`: temporary tokens and sessions, expiry and table limits.

use super::*;

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

mod temp {
    use super::*;

    #[test]
    fn issued_is_well_formed() {
        let mut t = Tickets::new();
        let tok = t.issue_temp(Instant::now()).unwrap();
        assert!(crate::web_gui::token::is_well_formed(&tok));
    }

    #[test]
    fn taken_once_then_refused() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        let tok = t.issue_temp(t0).unwrap();
        assert!(t.take_temp(&tok, t0 + secs(1)));
        assert!(!t.take_temp(&tok, t0 + secs(2)));
    }

    #[test]
    fn alive_at_599s_dead_at_600s() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        let a = t.issue_temp(t0).unwrap();
        let b = t.issue_temp(t0).unwrap();
        assert!(t.take_temp(&a, t0 + secs(599)));
        assert!(!t.take_temp(&b, t0 + secs(600)));
    }

    #[test]
    fn unknown_is_refused() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        t.issue_temp(t0).unwrap();
        assert!(!t.take_temp("0000000000000000000000000000000000000000000000000000", t0));
        assert!(!t.take_temp("", t0));
    }

    #[test]
    fn ninth_evicts_first_issued() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        let first = t.issue_temp(t0).unwrap();
        let mut rest = Vec::new();
        for i in 1..9u64 {
            rest.push(t.issue_temp(t0 + secs(i)).unwrap());
        }
        assert!(!t.take_temp(&first, t0 + secs(10)));
        assert!(t.take_temp(&rest[0], t0 + secs(10)));
        assert!(t.take_temp(&rest[7], t0 + secs(10)));
    }

    #[test]
    fn now_before_issue_does_not_panic() {
        let t0 = Instant::now() + secs(100);
        let mut t = Tickets::new();
        let tok = t.issue_temp(t0).unwrap();
        assert!(t.take_temp(&tok, t0 - secs(50)));
    }
}

mod session {
    use super::*;

    #[test]
    fn opened_returns_its_via() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        let id = t.open_session(Via::TempToken, t0).unwrap();
        assert_eq!(t.session(&id, t0), Some(Via::TempToken));
        assert_eq!(Via::Password.as_str(), "password");
        assert_eq!(Via::PermanentToken.as_str(), "token");
        assert_eq!(Via::TempToken.as_str(), "temp");
    }

    #[test]
    fn ids_differ() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        let a = t.open_session(Via::PermanentToken, t0).unwrap();
        let b = t.open_session(Via::PermanentToken, t0).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn alive_until_7_days_minus_1s() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        let id = t.open_session(Via::PermanentToken, t0).unwrap();
        assert_eq!(
            t.session(&id, t0 + secs(604_799)),
            Some(Via::PermanentToken)
        );
    }

    #[test]
    fn dead_at_7_days() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        let id = t.open_session(Via::PermanentToken, t0).unwrap();
        assert_eq!(t.session(&id, t0 + secs(604_800)), None);
    }

    #[test]
    fn closed_is_gone() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        let id = t.open_session(Via::PermanentToken, t0).unwrap();
        t.close_session(&id);
        assert_eq!(t.session(&id, t0), None);
    }

    #[test]
    fn close_unknown_is_noop() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        let id = t.open_session(Via::PermanentToken, t0).unwrap();
        t.close_session("no such session");
        assert_eq!(t.session(&id, t0), Some(Via::PermanentToken));
    }

    #[test]
    fn close_all_except_keeps_only_given() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        let keep = t.open_session(Via::Password, t0).unwrap();
        let other = t.open_session(Via::PermanentToken, t0).unwrap();
        t.close_sessions_except(Some(&keep));
        assert_eq!(t.session(&keep, t0), Some(Via::Password));
        assert_eq!(t.session(&other, t0), None);
    }

    #[test]
    fn close_all_except_none_closes_all() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        let a = t.open_session(Via::Password, t0).unwrap();
        t.close_sessions_except(None);
        assert_eq!(t.session(&a, t0), None);
    }

    #[test]
    fn sixty_fifth_evicts_earliest() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        let ids: Vec<String> = (0..65u64)
            .map(|i| t.open_session(Via::PermanentToken, t0 + secs(i)).unwrap())
            .collect();
        let now = t0 + secs(100);
        assert_eq!(t.session(&ids[0], now), None);
        assert_eq!(t.session(&ids[1], now), Some(Via::PermanentToken));
        assert_eq!(t.session(&ids[64], now), Some(Via::PermanentToken));
    }

    #[test]
    fn temps_survive_close_all() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        let tok = t.issue_temp(t0).unwrap();
        t.close_sessions_except(None);
        assert!(t.take_temp(&tok, t0));
    }

    #[test]
    fn drop_temps_refuses_all_issued() {
        let t0 = Instant::now();
        let mut t = Tickets::new();
        let a = t.issue_temp(t0).unwrap();
        let b = t.issue_temp(t0).unwrap();
        t.drop_temp_tokens();
        assert!(!t.take_temp(&a, t0));
        assert!(!t.take_temp(&b, t0));
    }
}
