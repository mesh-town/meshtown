//! Choosing the node's transports by the config's `net.transports`.
//!
//! What it does: checks names against `net_names::transports()` (and `mem` with `NodeConfig.test_net`) and returns
//! an `Arc<dyn Transport>` list in config order; `bind_ephemeral` is a one-off transport for redeeming an invite.
//! What it doesn't do: build the stack.

use std::sync::Arc;

use anyhow::{bail, Result};

use crate::net::mem::MemNet;
use crate::net::{NodeSecret, Transport};
use crate::node::config::NodeConfig;
use crate::node::net_names;
use crate::output::say;

use super::transports_mem::KeyedMem;

/// Parameters for bringing up transports; in production the default, tests pass the loopback cluster's bootstrap.
#[derive(Clone, Debug, Default)]
pub(super) struct BindOpts {
    #[cfg(feature = "net-hyper")]
    pub(super) hyper: crate::net_hyper::HyperOpts,
}

/// The transports brought up: a list for the stack and a typed `hyper` handle (for admission and DHT records).
pub(super) struct Bound {
    pub list: Vec<Arc<dyn Transport>>,
    #[cfg(feature = "net-hyper")]
    pub hyper: Option<Arc<crate::net_hyper::HyperTransport>>,
}

/// The node's transports in `net.transports` order; the default is `["hyper"]`.
pub(super) async fn bind_transports(
    cfg: &NodeConfig,
    secret: &NodeSecret,
) -> Result<Bound> {
    bind_with(cfg, secret, &BindOpts::default()).await
}

/// The same with explicit parameters; the list of transports brought up goes to the log.
pub(super) async fn bind_with(
    cfg: &NodeConfig,
    secret: &NodeSecret,
    opts: &BindOpts,
) -> Result<Bound> {
    let names = &cfg.settings.net_choice.transports;
    check_transport_names(names, cfg.test_net.is_some())?;
    let mut bound = Bound {
        list: Vec::with_capacity(names.len()),
        #[cfg(feature = "net-hyper")]
        hyper: None,
    };
    for name in names {
        add_by_name(&mut bound, name, (cfg, secret), opts).await?;
    }
    say("node", format!("transports: {}", names.join(", ")));
    Ok(bound)
}

/// The list is non-empty; the names are built into this build, `mem` only with `allow_mem`.
fn check_transport_names(names: &[String], allow_mem: bool) -> Result<()> {
    if names.is_empty() {
        bail!("the transport list is empty");
    }
    let have = net_names::transports();
    let known = |n: &String| have.contains(&n.as_str()) || (n == "mem" && allow_mem);
    if names.iter().any(|n| n == "mem") && !allow_mem {
        bail!("transport \"mem\" is for tests only");
    }
    if let Some(bad) = names.iter().find(|n| !known(n)) {
        bail!("transport \"{bad}\" is not built; available: {}", have.join(", "));
    }
    Ok(())
}

/// A transport by a checked name is appended to `bound`.
async fn add_by_name(
    bound: &mut Bound,
    name: &str,
    (cfg, secret): (&NodeConfig, &NodeSecret),
    opts: &BindOpts,
) -> Result<()> {
    match (name, &cfg.test_net) {
        ("mem", Some(net)) => bound.list.push(KeyedMem::new(net, secret)),
        #[cfg(feature = "net-hyper")]
        ("hyper", _) => {
            let hyper = open_hyper(secret, opts).await?;
            bound.list.push(hyper.clone());
            bound.hyper = Some(hyper);
        }
        (other, _) => {
            let _ = (secret, opts, &bound);
            bail!("transport \"{other}\" can't build this node")
        }
    }
    Ok(())
}

/// A one-off transport for redeeming an invite before the node: a fresh key, `hyper` (with `test_net` — `KeyedMem`),
/// waits until it is ready. Serves no services, sets no admission.
pub async fn bind_ephemeral(test_net: Option<&Arc<MemNet>>) -> Result<Arc<dyn Transport>> {
    open_ephemeral(test_net)
        .await
        .map_err(|e| anyhow::anyhow!("temporary transport did not come up: {e}"))
}

async fn open_ephemeral(test_net: Option<&Arc<MemNet>>) -> Result<Arc<dyn Transport>> {
    let secret = crate::config_file::new_secret()?;
    if let Some(net) = test_net {
        return Ok(KeyedMem::new(net, &secret));
    }
    ephemeral_hyper(secret).await
}

#[cfg(feature = "net-hyper")]
async fn ephemeral_hyper(secret: NodeSecret) -> Result<Arc<dyn Transport>> {
    let opts = crate::net_hyper::HyperOpts { watch_net: false, ..Default::default() };
    let hyper = crate::net_hyper::HyperTransport::bind(secret, opts).await?;
    hyper.wait_listen_outcome().await?;
    Ok(hyper)
}

#[cfg(not(feature = "net-hyper"))]
async fn ephemeral_hyper(_secret: NodeSecret) -> Result<Arc<dyn Transport>> {
    bail!("transport hyper is not built")
}

/// `hyper` is listed in the config explicitly, so a bind or announce refusal is a startup error.
#[cfg(feature = "net-hyper")]
async fn open_hyper(secret: &NodeSecret, opts: &BindOpts) -> Result<Arc<crate::net_hyper::HyperTransport>> {
    let hyper = crate::net_hyper::HyperTransport::bind(secret.clone(), crate::net_hyper::HyperOpts { watch_net: !cfg!(test), ..opts.hyper.clone() })
        .await
        .map_err(|e| anyhow::anyhow!("transport hyper did not come up: {e}"))?;
    hyper.set_admit(Some(super::transports_hyper::auth_invite_only()));
    hyper
        .wait_listen_outcome()
        .await
        .map_err(|e| anyhow::anyhow!("transport hyper did not announce itself in the DHT: {e}"))?;
    Ok(hyper)
}

#[cfg(test)]
#[path = "transports_tests.rs"]
mod tests;
