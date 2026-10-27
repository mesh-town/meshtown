//! The `net.relay` value kind: a string, `off` or `network`.

use serde_json::Value;

use super::super::parse_fields::push_field_error;
use super::super::relay_mode::RelayMode;
use super::super::report::FieldError;
use super::super::settings::Settings;
use super::row::Block;
use super::value::FieldValue;

/// `net.relay`: `"off"` or `"network"`.
pub(crate) struct NetRelay;

impl FieldValue for NetRelay {
    fn read_from_file(
        &self,
        field_path: &'static str,
        file_value: Option<&Value>,
        s: &mut Settings,
        errs: &mut Vec<FieldError>,
    ) {
        let Some(v) = file_value else { return };
        if let Some(mode) = v.as_str().and_then(RelayMode::parse) {
            s.net.relay = mode;
            return;
        }
        push_field_error(
            errs,
            field_path.to_string(),
            v,
            &self.expected(),
            RelayMode::default().as_str(),
        );
    }

    fn expected(&self) -> String {
        "off or network".to_string()
    }

    fn copy_value_between(&self, from: &Settings, to: &mut Settings) {
        to.net.relay = from.net.relay;
    }

    /// Written only when it differs from the default.
    fn render_for_file(&self, s: &Settings) -> Option<Value> {
        (s.net.relay != RelayMode::default()).then(|| Value::from(s.net.relay.as_str()))
    }

    fn debug_text(&self, s: &Settings) -> String {
        format!("{:?}", s.net.relay)
    }

    fn is_stored_in_settings(&self) -> bool {
        true
    }

    fn block(&self) -> Block {
        Block::Net
    }
}
