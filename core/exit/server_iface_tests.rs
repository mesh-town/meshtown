//! Exit streams against the `exit.interface` setting on the mem stack (`server.rs`): the target connection goes
//! through the chosen interface or is refused with code 8, and the refusal is not sent anywhere else.
//! The fixture is the one of `server_service_tests.rs`; the target is a real local site on `127.0.0.1`.

use super::*;
use crate::exit::frame::EXIT_CODE_INTERFACE_DOWN;
use crate::exit::{UDPGW_HOST, UDPGW_PORT};
use crate::iface_bind::test_support::{loopback_name, MISSING_IFACE};
use crate::iface_bind::{check, BindError};
use std::time::Duration;

use super::service_fixture::*;

const OPENED: [u8; 2] = [EXIT_PROTOCOL_VERSION, EXIT_CODE_OK];
const INTERFACE_DOWN: [u8; 2] = [EXIT_PROTOCOL_VERSION, EXIT_CODE_INTERFACE_DOWN];
const REFUSED: [u8; 2] = [EXIT_PROTOCOL_VERSION, EXIT_CODE_REFUSED];

mod opens {
    use super::*;

    #[tokio::test]
    async fn unset_interface_opens_as_before() {
        // given: no interface is set and the site is up
        let fx = setup_via(None, Opts::default()).await;

        // when: a member asks for a stream
        let (_pair, reply) = fx.ask(&request("site.test", 80)).await;

        // then: the code is `EXIT_CODE_OK` and the site accepted one connection
        assert_eq!(reply, OPENED);
        assert_eq!(accepted(&fx).await, 1);
    }

    #[tokio::test]
    async fn bound_to_loopback_opens() {
        // given: the exit is bound to the loopback interface
        let Some(name) = bindable_loopback() else {
            return;
        };
        let fx = setup_via(Some(&name), Opts::default()).await;

        // when: a member asks for a stream
        let (_pair, reply) = fx.ask(&request("site.test", 80)).await;

        // then: the code is `EXIT_CODE_OK` and the site accepted one connection
        assert_eq!(reply, OPENED);
        assert_eq!(accepted(&fx).await, 1);
    }

    #[tokio::test]
    async fn udpgw_stream_opens_even_when_the_interface_is_missing() {
        // given: the interface is missing (the datagrams are refused later, per session)
        let fx = setup_via(Some(MISSING_IFACE), Opts::default()).await;

        // when: a member opens the UDP gateway stream
        let (_pair, reply) = fx.ask(&request(UDPGW_HOST, UDPGW_PORT)).await;

        // then: the stream opens
        assert_eq!(reply, OPENED);
    }
}

mod refuses {
    use super::*;

    #[tokio::test]
    async fn missing_interface_refused_code_8() {
        // given: the exit is bound to an interface that doesn't exist
        let fx = setup_via(Some(MISSING_IFACE), Opts::default()).await;

        // when: a member asks for a stream
        let (_pair, reply) = fx.ask(&request("site.test", 80)).await;

        // then: the reply is code 8 and the site saw no connection
        assert_eq!(reply, INTERFACE_DOWN);
        assert_eq!(accepted(&fx).await, 0, "a refused stream must not dial");
    }

    #[tokio::test]
    async fn live_switch_applies_to_next_stream() {
        // given: a stream opened through loopback
        let Some(name) = bindable_loopback() else {
            return;
        };
        let fx = setup_via(Some(&name), Opts::default()).await;
        let (_first, reply) = fx.ask(&request("site.test", 80)).await;
        assert_eq!(reply, OPENED);

        // when: the interface is switched to a missing one and another stream is asked for
        fx.cfg.iface.set(Some(MISSING_IFACE));
        let (_second, reply) = fx.ask(&request("site.test", 80)).await;

        // then: the next stream gets code 8 and only the first one reached the site
        assert_eq!(reply, INTERFACE_DOWN);
        assert_eq!(accepted(&fx).await, 1);
    }

    #[tokio::test]
    async fn non_member_still_refused_first() {
        // given: the interface is missing and the client is not a member
        let fx = setup_via(
            Some(MISSING_IFACE),
            Opts {
                member: false,
                ..Opts::default()
            },
        )
        .await;

        // when: it asks for a stream
        let (_pair, reply) = fx.ask(&request("site.test", 80)).await;

        // then: the reply is `EXIT_CODE_REFUSED`, the permission check comes before the interface
        assert_eq!(reply, REFUSED);
        assert_eq!(accepted(&fx).await, 0);
    }
}

/// The fixture with the exit's interface set before the first request (the handler shares it with `fx.cfg`).
async fn setup_via(iface: Option<&str>, opts: Opts) -> Fx {
    let fx = setup(opts).await;
    fx.cfg.iface.set(iface);
    fx
}

/// Connections the site has accepted, after a short wait for the accept loop to run.
async fn accepted(fx: &Fx) -> usize {
    tokio::time::sleep(Duration::from_millis(100)).await;
    fx.site_accepts()
}

/// The loopback interface name, or `None` (the test is skipped) where the kernel refuses binding to a device.
fn bindable_loopback() -> Option<String> {
    let name = loopback_name();
    match check(&name) {
        Ok(()) => Some(name),
        Err(BindError::NoPermission(_)) => {
            eprintln!("skipped: binding to a device needs CAP_NET_RAW on this kernel");
            None
        }
        Err(e) => panic!("the loopback interface {name} must be usable: {e}"),
    }
}
