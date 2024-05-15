use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context, Error};
use cln_plugin::Plugin;
use cln_rpc::{
    model::{
        requests::{ListchannelsRequest, ListnodesRequest},
        responses::ListnodesNodesAddressesType,
    },
    primitives::{Amount, PublicKey},
    ClnRpc,
};
use log::debug;
use serde_json::{json, Value};
use tokio::time;

use crate::structs::{
    AmbossResponse, ChannelFlags, OneMl, PeerData, PeerDataCache, PeerInfo, PluginState,
};

async fn get_oneml_data(
    pubkey: PublicKey,
    network: String,
    oneml_lock: Arc<tokio::sync::Mutex<u128>>,
) -> Result<OneMl, Error> {
    let mut last_api_call = oneml_lock.lock().await;
    debug!("oneml_data: start");

    let mut unix_now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    while (unix_now_ms - *last_api_call) <= 1000 {
        time::sleep(Duration::from_millis(100)).await;
        unix_now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
    }

    let response = match network {
        name if name.eq_ignore_ascii_case("bitcoin") => {
            reqwest::get(format!("https://1ml.com/node/{}/json", pubkey)).await?
        }
        name if name.eq_ignore_ascii_case("testnet") => {
            reqwest::get(format!("https://1ml.com/testnet/node/{}/json", pubkey)).await?
        }
        _ => return Err(anyhow!("network not supported for 1ML: {}", network)),
    };

    *last_api_call = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    debug!("oneml_data: done");

    if response.status().is_success() {
        let json: Value = response.json().await?;
        let noderank = json
            .get("noderank")
            .context(format!("1ML: {} has no ranks!", pubkey))?;
        let one_ml_ranks: OneMl = serde_json::from_value(noderank.clone())?;
        Ok(one_ml_ranks)
    } else {
        Err(anyhow!(
            "1ML: bad API response, status:{}",
            response.status()
        ))
    }
}

async fn get_amboss_data(
    pubkey: PublicKey,
    network: String,
    amboss_lock: Arc<tokio::sync::Mutex<u128>>,
) -> Result<AmbossResponse, Error> {
    let mut last_api_call = amboss_lock.lock().await;
    debug!("amboss_data: start");

    let query = "query ExampleQuery($pubkey: String!) {
        getNode(pubkey: $pubkey) {
          graph_info {
            metrics {
              capacity_rank
              channels_rank
            }
          }
          socials {
            info {
              email
              linkedin
              nostr
              telegram
              twitter
              website
            }
            lightning_labs {
              terminal_web {
                position
              }
            }
          }
        }
      }
      ";

    let mut unix_now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    // cost ist 673 with recovery of 500 -> roughly 1400ms
    while (unix_now_ms - *last_api_call) <= 1400 {
        time::sleep(Duration::from_millis(100)).await;
        unix_now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
    }

    let response = match network {
        name if name.eq_ignore_ascii_case("bitcoin") => {
            reqwest::Client::new()
                .post("https://api.amboss.space/graphql")
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .json(&json!({"query":query, "variables":{"pubkey":pubkey.to_string()}}))
                .send()
                .await?
        }
        _ => return Err(anyhow!("network not supported for Amboss: {}", network)),
    };

    *last_api_call = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    debug!("amboss_data: done");

    if response.status().is_success() {
        let json_response: serde_json::Value = response.json().await?;
        debug!("{:?}", json_response);
        let amboss_response: AmbossResponse = serde_json::from_value(json_response)?;

        Ok(amboss_response)
    } else {
        Err(anyhow!(
            "Amboss: bad API response, status:{}",
            response.status()
        ))
    }
}

async fn get_gossip_data(
    rpc_path: PathBuf,
    pubkey: PublicKey,
    their_funding_msat: Amount,
    channel_flags: ChannelFlags,
) -> Result<PeerInfo, Error> {
    debug!("gossip_data: start");
    let mut list_node_rpc = ClnRpc::new(&rpc_path).await?;

    let list_node_task = tokio::spawn(async move {
        list_node_rpc
            .call_typed(&ListnodesRequest { id: Some(pubkey) })
            .await
    });

    let mut list_channels_rpc = ClnRpc::new(&rpc_path).await?;
    let list_channels_task = tokio::spawn(async move {
        list_channels_rpc
            .call_typed(&ListchannelsRequest {
                short_channel_id: None,
                source: Some(pubkey),
                destination: None,
            })
            .await
    });

    let list_nodes = list_node_task.await??.nodes;
    let list_node = if let Some(node) = list_nodes.first() {
        debug!("{:?}", node);
        node
    } else {
        return Err(anyhow!("no node found for {}", pubkey));
    };
    let list_channels = list_channels_task.await??.channels;

    let peerinfo = PeerInfo {
        pubkey,
        their_funding_sat: their_funding_msat.msat() / 1_000,
        channel_flags,
        channel_count: Some(list_channels.len() as u64),
        node_capacity_sat: Some(
            list_channels
                .iter()
                .map(|c| c.amount_msat.msat() / 1000)
                .sum(),
        ),
        has_clearnet: Some(list_node.addresses.as_ref().map_or(false, |a| {
            a.iter().any(|t| {
                t.item_type == ListnodesNodesAddressesType::DNS
                    || t.item_type == ListnodesNodesAddressesType::IPV4
                    || t.item_type == ListnodesNodesAddressesType::IPV6
            })
        })),
        has_tor: Some(list_node.addresses.as_ref().map_or(false, |a| {
            a.iter().any(|t| {
                t.item_type == ListnodesNodesAddressesType::TORV2
                    || t.item_type == ListnodesNodesAddressesType::TORV3
            })
        })),
        anchor_support: Some(check_feature(
            if let Some(features) = &list_node.features {
                features
            } else {
                return Err(anyhow!("node has no features?!"));
            },
            vec![22, 23],
        )?),
    };
    debug!("gossip_data: done");
    Ok(peerinfo)
}

pub async fn collect_data(
    plugin: &Plugin<PluginState>,
    pubkey: PublicKey,
    their_funding_msat: Amount,
    channel_flags: ChannelFlags,
    custom_rule: &str,
) -> Result<PeerData, Error> {
    debug!("collect_data: start");
    let unix_now_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    {
        if let Some(cache) = plugin.state().peerdata_cache.lock().get(&pubkey) {
            if unix_now_s - cache.age <= 3600 {
                return Ok(cache.peer_data.clone());
            }
        }
    }
    let rpc_path =
        Path::new(&plugin.configuration().lightning_dir).join(plugin.configuration().rpc_file);
    let network = plugin.configuration().network;

    debug!("collect_data: custom_rule: {}", custom_rule);
    let gossip_task = if custom_rule.to_ascii_lowercase().contains("cln_") {
        let channel_flags_clone = channel_flags.clone();
        Some(tokio::spawn(async move {
            get_gossip_data(rpc_path, pubkey, their_funding_msat, channel_flags_clone).await
        }))
    } else {
        None
    };

    let amboss_task = if custom_rule.to_ascii_lowercase().contains("amboss_") {
        let network_amboss = network.clone();
        let amboss_lock = plugin.state().amboss_lock.clone();
        Some(tokio::spawn(async move {
            let mut attempts = 1;
            loop {
                let result =
                    get_amboss_data(pubkey, network_amboss.clone(), amboss_lock.clone()).await;
                if result.is_ok() || attempts >= 3 {
                    break result;
                }
                time::sleep(Duration::from_secs(attempts * 2)).await;
                attempts += 1;
            }
        }))
    } else {
        None
    };

    let oneml_task = if custom_rule.to_ascii_lowercase().contains("oneml_") {
        let oneml_lock = plugin.state().oneml_lock.clone();
        Some(tokio::spawn(async move {
            let mut attempts = 1;
            loop {
                let result = get_oneml_data(pubkey, network.clone(), oneml_lock.clone()).await;
                if result.is_ok() || attempts >= 3 {
                    break result;
                }
                time::sleep(Duration::from_secs(attempts * 2)).await;
                attempts += 1;
            }
        }))
    } else {
        None
    };

    let peerinfo = if let Some(gdata) = gossip_task {
        gdata.await??
    } else {
        PeerInfo {
            pubkey,
            their_funding_sat: their_funding_msat.msat() / 1_000,
            channel_flags,
            channel_count: None,
            node_capacity_sat: None,
            has_clearnet: None,
            has_tor: None,
            anchor_support: None,
        }
    };
    debug!("collect_data: peerinfo: {:?}", peerinfo);

    let amboss_data = if let Some(ad) = amboss_task {
        Some(ad.await??.data)
    } else {
        None
    };

    let oneml_data = if let Some(ml) = oneml_task {
        Some(ml.await??)
    } else {
        None
    };

    let peer_data = PeerData {
        peerinfo,
        oneml_data,
        amboss_data,
    };

    let mut cache = plugin.state().peerdata_cache.lock();
    cache.insert(
        pubkey,
        PeerDataCache {
            peer_data: peer_data.clone(),
            age: unix_now_s,
        },
    );
    debug!("collect_data: done");
    Ok(peer_data)
}

fn check_feature(hex: &str, check_bits: Vec<u16>) -> Result<bool, Error> {
    let mut bits = Vec::new();
    for hex_char in hex.chars() {
        let binary_string = match hex_char.to_digit(16) {
            Some(n) => format!("{:04b}", n),
            None => {
                return Err(anyhow!("Invalid hexadecimal character: {}", hex_char));
            }
        };
        for bit in binary_string.chars() {
            bits.push(bit == '1');
        }
    }
    // debug!("binary: {:?}", bits);
    let mut result = false;
    for bit in check_bits {
        if let Some(b) = bits.get(bits.len() - 1 - bit as usize) {
            debug!("found bit {}: {}", bit, b);
            result = result || *b;
        } else {
            return Err(anyhow!("bit not found"));
        }
    }
    Ok(result)
}
