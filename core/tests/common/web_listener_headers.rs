//! Response headers for the web listener scenarios that look at them.
//!
//! What it does: adds `Resp::headers` (names in lower case) on top of the base response.
//! What it does not do: it does not send anything.
//! Links: included as a sibling of `common` by the scenarios that read headers; takes `Resp` through `super::common`.

use std::collections::HashMap;

use super::common::Resp;

impl Resp {
    /// The response headers, names in lower case.
    pub(crate) fn headers(&self) -> HashMap<String, String> {
        let head = String::from_utf8_lossy(&self.head);
        let mut headers = HashMap::new();
        for line in head.split("\r\n").skip(1) {
            if let Some((k, v)) = line.split_once(':') {
                headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
            }
        }
        headers
    }
}
