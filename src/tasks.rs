use std::{
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use cln_plugin::Plugin;
use cln_rpc::{ClnRpc, model::requests::ListnodesRequest};

use crate::{abuse::evict_abuse, collect::CACHE_TTL, structs::PluginState};

pub async fn refresh_alias_cache(plugin: Plugin<PluginState>) -> Result<(), anyhow::Error> {
    let now = Instant::now();
    log::debug!("Starting refresh_alias_cache task");
    plugin.state().alias_cache.lock().clear();

    let rpc_path =
        Path::new(&plugin.configuration().lightning_dir).join(plugin.configuration().rpc_file);
    let mut rpc = ClnRpc::new(&rpc_path).await?;

    let listnodes = rpc.call_typed(&ListnodesRequest { id: None }).await?.nodes;
    let mut alias_cache = plugin.state().alias_cache.lock();
    for node in listnodes {
        if let Some(a) = node.alias {
            alias_cache.insert(node.nodeid, a);
        }
    }

    log::debug!(
        "refresh_alias_cache done in: {}ms",
        now.elapsed().as_millis()
    );
    Ok(())
}

pub fn evict_cache(plugin: &Plugin<PluginState>) {
    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut caches = plugin.state().peerdata_cache.lock();
    caches.retain(|_, v| now_unix - v.age <= CACHE_TTL);

    evict_abuse(&plugin.state().abuse_cache);
}
