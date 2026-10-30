//! `access` without an action: prints who can reach the page and the proxy and on which networks.

use serde_json::{json, Value};

use super::settings_io::{load, route, Route};
use crate::cli::args::{Global, Globals};
use crate::cli::commands::client::print_json;
use crate::config::config_file::{IfaceFilter, ListenAccess, Settings};

/// The four values of the command: both access modes and both interface lists.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct AccessView {
    pub(super) page: ListenAccess,
    pub(super) proxy: ListenAccess,
    pub(super) page_ifaces: IfaceFilter,
    pub(super) proxy_ifaces: IfaceFilter,
}

impl AccessView {
    pub(super) fn from_settings(s: &Settings) -> AccessView {
        AccessView {
            page: s.process.web_access,
            proxy: s.process.socks_access,
            page_ifaces: s.process.web_ifaces.clone(),
            proxy_ifaces: s.process.socks_ifaces.clone(),
        }
    }

    /// From the `config` document of `GET /api/config`; a missing or odd field gives `None`.
    pub(super) fn from_config(config: &Value) -> Option<AccessView> {
        let access = |section: &str| ListenAccess::parse(config[section]["access"].as_str()?);
        Some(AccessView {
            page: access("web")?,
            proxy: access("socks")?,
            page_ifaces: ifaces_of(&config["web"]["ifaces"])?,
            proxy_ifaces: ifaces_of(&config["socks"]["ifaces"])?,
        })
    }

    /// The two lists differ: `access ifaces` would set them equal.
    pub(super) fn ifaces_differ(&self) -> bool {
        self.page_ifaces != self.proxy_ifaces
    }

    /// The text lines, one `name value` pair each.
    pub(super) fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("page {}", self.page.as_str()),
            format!("proxy {}", self.proxy.as_str()),
        ];
        match self.ifaces_differ() {
            true => lines.extend([
                format!("page-ifaces {}", describe(&self.page_ifaces)),
                format!("proxy-ifaces {}", describe(&self.proxy_ifaces)),
            ]),
            false => lines.push(format!("ifaces {}", describe(&self.page_ifaces))),
        }
        lines
    }

    /// The same four paths and shapes as the `config` document of the node.
    pub(super) fn json(&self) -> Value {
        json!({
            "web": {"access": self.page.as_str(), "ifaces": self.page_ifaces.parts().1},
            "socks": {"access": self.proxy.as_str(), "ifaces": self.proxy_ifaces.parts().1},
        })
    }
}

/// `[]` is automatic, `["*"]` any, otherwise a list; anything else is not an interface list.
fn ifaces_of(value: &Value) -> Option<IfaceFilter> {
    let names = value
        .as_array()?
        .iter()
        .map(|n| n.as_str().map(str::to_string))
        .collect::<Option<Vec<_>>>()?;
    IfaceFilter::from_names(names)
}

fn describe(filter: &IfaceFilter) -> String {
    match filter {
        IfaceFilter::Auto => "auto".to_string(),
        IfaceFilter::Any => "*".to_string(),
        IfaceFilter::Only(names) => names.join(","),
    }
}

pub(super) fn run(g: &Globals) -> anyhow::Result<()> {
    g.allow("access", &[Global::Json])?;
    let view = match route(g)? {
        Route::File(path) => AccessView::from_settings(&load(&path, "access")?.settings),
        Route::Live(node) => from_node(&node.call("GET", "/api/config", None)?)?,
    };
    match g.json {
        true => print_json(&view.json()),
        false => print_lines(&view),
    }
    Ok(())
}

fn from_node(reply: &Value) -> anyhow::Result<AccessView> {
    AccessView::from_config(&reply["config"]).ok_or_else(|| {
        anyhow::anyhow!("the reply lacks the access settings (web.access, socks.access, ifaces)")
    })
}

fn print_lines(view: &AccessView) {
    println!("{}", view.lines().join("\n"));
    if view.ifaces_differ() {
        eprintln!(
            "The page and the proxy use different interface lists; `access ifaces` sets both."
        );
    }
}

#[cfg(test)]
#[path = "show_tests.rs"]
mod tests;
