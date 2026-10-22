//! The field value kind: how to read it from the file and what to say on a miss. A table row
//! holds `&'static dyn FieldValue`; an ordinary field is `SettingsSlot` (access to a `Settings` field + `Kind`).

use serde_json::Value;

use super::super::parse_fields::push_field_error;
use super::super::report::FieldError;
use super::super::settings::Settings;

/// Reads one table row from the file into `Settings`.
pub(crate) trait FieldValue: Sync {
    /// No key — `Settings` is unchanged (it already holds the default); a bad value — an entry in `errs`
    /// and the default stays; otherwise the value is written to `s`.
    fn read_from_file(
        &self,
        field_path: &'static str,
        file_value: Option<&Value>,
        s: &mut Settings,
        errs: &mut Vec<FieldError>,
    );
    /// What this row expects (also the text of the validation error).
    fn expected(&self) -> String;
    /// Moves this row's value from `from` to `to`.
    fn copy_value_between(&self, from: &Settings, to: &mut Settings);
    /// The value for the file; `None` — the key is not written.
    fn render_for_file(&self, s: &Settings) -> Option<Value>;
    /// The value for the settings `Debug`.
    fn debug_text(&self, s: &Settings) -> String;
    /// Whether the row is stored in `Settings` (a computed one is not).
    fn is_stored_in_settings(&self) -> bool;
}

/// The rule of one value kind of type `T`.
pub(crate) trait Kind<T>: Sync {
    /// The value from the file; `None` — it does not fit.
    fn parse(&self, v: &Value) -> Option<T>;
    /// What was expected, for the message "file has …, expected …".
    fn expected(&self) -> String;
    /// How the default looks in the message "using default …".
    fn default_text(&self, d: &T) -> String;
    /// The value for the file; `None` — the key is not written.
    fn to_json(&self, v: &T) -> Option<Value>;
}

/// A `Settings` field with kind `K`: read and write access without capturing, so the table row
/// is extended to `'static`.
pub(crate) struct SettingsSlot<T: 'static, K: 'static> {
    pub get: fn(&Settings) -> &T,
    pub set: fn(&mut Settings, T),
    pub kind: K,
}

impl<T: Clone + std::fmt::Debug + 'static, K: Kind<T> + 'static> FieldValue for SettingsSlot<T, K> {
    fn read_from_file(
        &self,
        field_path: &'static str,
        file_value: Option<&Value>,
        s: &mut Settings,
        errs: &mut Vec<FieldError>,
    ) {
        let Some(v) = file_value else { return };
        match self.kind.parse(v) {
            Some(x) => (self.set)(s, x),
            None => {
                let default = self.kind.default_text((self.get)(&Settings::default()));
                push_field_error(errs, field_path.to_string(), v, &self.kind.expected(), default);
            }
        }
    }

    fn expected(&self) -> String {
        self.kind.expected()
    }

    fn copy_value_between(&self, from: &Settings, to: &mut Settings) {
        (self.set)(to, (self.get)(from).clone());
    }

    fn render_for_file(&self, s: &Settings) -> Option<Value> {
        self.kind.to_json((self.get)(s))
    }

    fn debug_text(&self, s: &Settings) -> String {
        format!("{:?}", (self.get)(s))
    }

    fn is_stored_in_settings(&self) -> bool {
        true
    }
}

/// A computed row: the key is known (not an "unknown field"), but it is not in `Settings`.
pub(crate) struct Computed;

impl FieldValue for Computed {
    fn read_from_file(&self, _: &'static str, _: Option<&Value>, _: &mut Settings, _: &mut Vec<FieldError>) {}

    fn expected(&self) -> String {
        String::new()
    }

    fn copy_value_between(&self, _: &Settings, _: &mut Settings) {}

    fn render_for_file(&self, _: &Settings) -> Option<Value> {
        None
    }

    fn debug_text(&self, _: &Settings) -> String {
        String::new()
    }

    fn is_stored_in_settings(&self) -> bool {
        false
    }
}
