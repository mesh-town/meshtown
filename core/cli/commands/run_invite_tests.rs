//! Tests for `plan_invite`: whether to spend the invite in `run` — everything is decided by a pure
//! function before any trip to the network. The output reads as a phrase: `key_from_flag::of_another_network::…`.

use super::*;
use crate::invite::{InviteHead, InviteSecret, INVITE_TTL_SECS};
use crate::members::network_key::{managed_key_with_label, parse_user_input, NetworkKey};
use crate::transport::net::{NodeSecret, PeerId};

const NOW: u64 = 1_790_000_000;
const HOUR: u64 = 3_600;

/// The expired-invite text, verbatim from the invite module.
const EXPIRED_TEXT: &str =
    "The invite has run out: it is valid for a day, and no member let this computer in during that time. Ask whoever sent it for a new one.";

mod without_invite {
    use super::*;

    #[test]
    fn plans_nothing_whatever_the_keys() {
        // given
        let opts = Opts {
            with_invite: false,
            cli_key: Some(text_b()),
            file_key: Some(text_b()),
            ..Opts::default()
        };
        // when
        let out = plan(opts);
        // then
        assert!(matches!(out, Outcome::NoInvite), "{out:?}");
    }

    #[test]
    fn plans_nothing_without_keys_too() {
        let out = plan(Opts {
            with_invite: false,
            ..Opts::default()
        });
        assert!(matches!(out, Outcome::NoInvite), "{out:?}");
    }
}

mod without_any_key {
    use super::*;

    #[test]
    fn redeems_the_invite() {
        // given / when
        let out = plan(Opts::default());
        // then
        then_redeems_the_same_invite(out);
    }
}

mod key_from_flag {
    use super::*;

    mod of_the_same_network {
        use super::*;

        #[test]
        fn keeps_the_key_and_does_not_spend_the_invite() {
            // given
            let opts = Opts {
                cli_key: Some(text_a()),
                ..Opts::default()
            };
            // when
            let out = plan(opts);
            // then
            assert!(matches!(out, Outcome::KeepKey), "{out:?}");
        }

        #[test]
        fn keeps_the_key_written_with_a_label() {
            // given
            let opts = Opts {
                cli_key: Some(managed_key_with_label(
                    Some("our-game"),
                    &[9u8; 32],
                    &creator(),
                )),
                ..Opts::default()
            };
            // when
            let out = plan(opts);
            // then
            assert!(matches!(out, Outcome::KeepKey), "{out:?}");
        }
    }

    mod of_another_network {
        use super::*;

        #[test]
        fn refuses_and_says_the_invite_is_not_spent() {
            // given
            let opts = Opts {
                cli_key: Some(text_b()),
                ..Opts::default()
            };
            // when
            let text = plan(opts).refusal();
            // then
            assert!(text.contains("from different"), "{text}");
            assert!(text.contains("--key"), "{text}");
            assert!(text.contains("was not spent"), "{text}");
        }

        #[test]
        fn refuses_even_when_the_file_holds_another_network() {
            // given: the key from --key and the invite disagree whatever the file holds
            let opts = Opts {
                cli_key: Some(text_b()),
                file_key: Some(text_b()),
                ..Opts::default()
            };
            // when
            let text = plan(opts).refusal();
            // then
            assert!(text.contains("was not spent"), "{text}");
        }
    }
}

#[path = "run_invite_file_key_tests.rs"]
mod key_in_settings_file;

mod network_slots {
    use super::*;

    #[test]
    fn keeps_the_key_when_the_network_has_a_slot() {
        let out = plan(Opts {
            file_key: Some(text_b()),
            slot_keys: vec![None, Some(text_a())],
            ..Opts::default()
        });
        assert!(matches!(out, Outcome::KeepKey), "{out:?}");
    }

    #[test]
    fn refuses_at_the_limit_and_says_the_invite_is_not_spent() {
        let full = vec![Some(text_c()); MAX_NETS - 1];
        let text = plan(Opts {
            file_key: Some(text_b()),
            slot_keys: full,
            ..Opts::default()
        })
        .refusal();
        assert!(text.contains("too many networks"), "{text}");
        assert!(text.contains("was not spent"), "{text}");
    }

    #[test]
    fn adds_a_network_one_below_the_limit() {
        let almost = vec![Some(text_c()); MAX_NETS - 2];
        let out = plan(Opts {
            file_key: Some(text_b()),
            slot_keys: almost,
            ..Opts::default()
        });
        then_adds_a_network_with_the_same_invite(out);
    }
}

mod expired_invite {
    use super::*;

    #[test]
    fn refuses_with_the_expired_text_instead_of_redeeming() {
        // given
        let opts = Opts {
            now: NOW + 25 * HOUR,
            ..Opts::default()
        };
        // when
        let text = plan(opts).refusal();
        // then
        assert!(text.contains(EXPIRED_TEXT), "{text}");
    }

    #[test]
    fn refuses_at_exactly_the_ttl() {
        // given
        let opts = Opts {
            now: NOW + INVITE_TTL_SECS,
            ..Opts::default()
        };
        // when
        let text = plan(opts).refusal();
        // then
        assert!(text.contains(EXPIRED_TEXT), "{text}");
    }

    #[test]
    fn refuses_even_when_the_invite_would_add_a_network() {
        // given: without the expiry this would be a join as a separate network
        let opts = Opts {
            file_key: Some(text_b()),
            now: NOW + 25 * HOUR,
            ..Opts::default()
        };
        // when
        let text = plan(opts).refusal();
        // then
        assert!(text.contains(EXPIRED_TEXT), "{text}");
    }

    #[test]
    fn made_twenty_three_hours_ago_still_redeems() {
        // given
        let opts = Opts {
            now: NOW + 23 * HOUR,
            ..Opts::default()
        };
        // when
        let out = plan(opts);
        // then
        then_redeems_the_same_invite(out);
    }
}

// ---- helpers ----

/// Run preconditions: an invite of network `net_a()` issued at `NOW`, and what is in the keys.
struct Opts {
    with_invite: bool,
    cli_key: Option<String>,
    file_key: Option<String>,
    slot_keys: Vec<Option<String>>,
    now: u64,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            with_invite: true,
            cli_key: None,
            file_key: None,
            slot_keys: Vec::new(),
            now: NOW,
        }
    }
}

/// The `plan_invite` decision in a form that can be printed in a check message.
#[derive(Debug)]
enum Outcome {
    NoInvite,
    KeepKey,
    Redeem(ParsedInvite),
    AddNet(ParsedInvite),
    Refused(String),
}

impl Outcome {
    /// The refusal text; anything but a refusal fails the check.
    fn refusal(self) -> String {
        match self {
            Outcome::Refused(text) => text,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }
}

fn plan(opts: Opts) -> Outcome {
    let inv = opts.with_invite.then(invite_of_net_a);
    let step = plan_invite(
        inv,
        opts.cli_key.as_deref(),
        opts.file_key.as_deref(),
        &opts.slot_keys,
        opts.now,
    );
    match step {
        Ok(InviteStep::None) => Outcome::NoInvite,
        Ok(InviteStep::KeepKey) => Outcome::KeepKey,
        Ok(InviteStep::Redeem(p)) => Outcome::Redeem(p),
        Ok(InviteStep::AddNet(p)) => Outcome::AddNet(p),
        Err(e) => Outcome::Refused(format!("{e:#}")),
    }
}

/// The invite is spent, and it is exactly the one that was passed in.
fn then_redeems_the_same_invite(out: Outcome) {
    match out {
        Outcome::Redeem(p) => assert_is_the_invite(&p),
        other => panic!("expected Redeem, got {other:?}"),
    }
}

/// The invite is spent for a separate network, and it is exactly the one that was passed in.
fn then_adds_a_network_with_the_same_invite(out: Outcome) {
    match out {
        Outcome::AddNet(p) => assert_is_the_invite(&p),
        other => panic!("expected AddNet, got {other:?}"),
    }
}

fn assert_is_the_invite(p: &ParsedInvite) {
    assert_eq!(p.head.secret, secret(), "a different invite went in");
    assert_eq!(p.head.made_unix, NOW);
}

fn secret() -> InviteSecret {
    InviteSecret::from_bytes([0xC7; 32])
}

/// The creator of the test networks.
fn creator() -> PeerId {
    NodeSecret::from_bytes(&[1u8; 32]).peer_id()
}

fn text_of(seed: u8) -> String {
    managed_key_with_label(None, &[seed; 32], &creator())
}

fn text_a() -> String {
    text_of(9)
}

fn text_b() -> String {
    text_of(8)
}

fn text_c() -> String {
    text_of(7)
}

fn net_a() -> NetworkKey {
    parse_user_input(&text_a()).unwrap().key
}

fn invite_of_net_a() -> ParsedInvite {
    ParsedInvite {
        head: InviteHead {
            secret: secret(),
            network_id: net_a().subkeys().network_id,
            label: None,
            made_unix: NOW,
            root: creator(),
        },
        peers: vec![],
        warnings: vec![],
    }
}

// ============================================================================================
// Writing the received key to the settings file (`persist_key`, `write_key`)
// ============================================================================================

#[path = "run_invite_persist_tests.rs"]
mod persisting_the_key;
