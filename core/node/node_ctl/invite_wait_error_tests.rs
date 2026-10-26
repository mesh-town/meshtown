//! Tests of `make_error`: which refusals of `invite::make` get their own page code.

use super::make_error;
use crate::invite::{NotAdmin, OutOfNetwork};
use crate::web_gui::ActionError;

fn denied_code(e: anyhow::Error) -> &'static str {
    match make_error(e) {
        ActionError::Denied { code, .. } => code,
        other => panic!("expected Denied, got {other:?}"),
    }
}

#[test]
fn not_admin_is_denied_with_its_code() {
    assert_eq!(denied_code(NotAdmin.into()), "not_admin");
}

#[test]
fn out_of_network_is_denied_with_its_code() {
    assert_eq!(denied_code(OutOfNetwork.into()), "not_in_network");
}

#[test]
fn out_of_network_keeps_its_text() {
    let ActionError::Denied { message, .. } = make_error(OutOfNetwork.into()) else {
        panic!("expected Denied");
    };
    assert_eq!(message, OutOfNetwork.to_string());
}

#[test]
fn other_errors_are_internal() {
    let result = make_error(anyhow::anyhow!("boom"));
    assert!(matches!(result, ActionError::Internal(_)), "got {result:?}");
}
