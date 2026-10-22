//! Tests for choosing a link on simultaneous dials: both sides decide the same way.

use super::keeps_new;
use crate::transport::net::facade::test_support::pid;

mod keeps_new {
    use super::*;

    /// given: two links, one dialed by A (the smaller), the other by B → when: both sides decide which one stays
    /// in any arrival order → then: the decisions match and the smaller one's link stays.
    #[test]
    fn smaller_dialer_wins() {
        let (a, b) = (pid(1).min(pid(2)), pid(1).max(pid(2)));

        // A's link arrived first, B's second: the new one (B) loses on both sides
        assert!(!keeps_new(a, b, a, b));
        assert!(!keeps_new(b, a, a, b));
        // B's link arrived first, A's second: the new one (A) wins on both sides
        assert!(keeps_new(a, b, b, a));
        assert!(keeps_new(b, a, b, a));
    }

    /// given: the current and the new one were dialed by the same side → when: both sides decide → then: the new one wins on both.
    #[test]
    fn same_dialer_new_wins() {
        let (a, b) = (pid(1).min(pid(2)), pid(1).max(pid(2)));

        for dialer in [a, b] {
            assert!(keeps_new(a, b, dialer, dialer));
            assert!(keeps_new(b, a, dialer, dialer));
        }
    }
}
