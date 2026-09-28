use bitreq::Proxy;
use cln_plugin::ConfiguredPlugin;

use crate::structs::PluginState;

/// Assumes values is not empty and sorted
pub fn median(values: &[u16]) -> u16 {
    let mid = values.len() / 2;

    if values.len() % 2 == 1 {
        values[mid]
    } else {
        u16::midpoint(values[mid - 1], values[mid])
    }
}

pub fn get_proxy(
    plugin: &ConfiguredPlugin<PluginState, tokio::io::Stdin, tokio::io::Stdout>,
) -> Result<Option<Proxy>, anyhow::Error> {
    let Some(use_proxy) = plugin.configuration().always_use_proxy else {
        return Ok(None);
    };
    if !use_proxy {
        return Ok(None);
    }

    let Some(proxy_info) = plugin.configuration().proxy else {
        return Ok(None);
    };
    Ok(Some(Proxy::new_socks5(format!(
        "socks5h://{}:{}",
        proxy_info.address, proxy_info.port
    ))?))
}
