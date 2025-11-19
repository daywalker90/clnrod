use std::{path::Path, time::Instant};

use cln_plugin::Plugin;
use cln_rpc::{model::requests::ListnodesRequest, ClnRpc};

use crate::structs::PluginState;

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
