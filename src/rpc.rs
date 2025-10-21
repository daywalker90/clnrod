use std::{path::Path, str::FromStr};

use anyhow::{anyhow, Context, Error};
use cln_plugin::Plugin;
use cln_rpc::{
    model::requests::ConnectRequest,
    primitives::{Amount, PublicKey},
    ClnRpc,
};
use serde_json::json;

use crate::{
    collect::{collect_data, ln_ping},
    config::read_pubkey_list,
    notify::notify,
    parser::{evaluate_rule, parse_rule},
    structs::{BlockMode, ChannelFlags, ClnrodParser, NotifyVerbosity, PluginState},
    OPT_BLOCK_MODE, PLUGIN_NAME,
};

pub async fn clnrod_reload(
    plugin: Plugin<PluginState>,
    _args: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let plugin_dir = Path::new(&plugin.configuration().lightning_dir).join(PLUGIN_NAME);
    let block_mode = BlockMode::from_str(
        plugin
            .option_str(OPT_BLOCK_MODE)
            .unwrap()
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let (removed, added) =
        read_pubkey_list(plugin.state().pubkey_list.clone(), plugin_dir, block_mode).await?;

    Ok(json!({"removed":removed, "added":added}))
}

pub async fn clnrod_testrule(
    plugin: Plugin<PluginState>,
    args: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let config = plugin.state().config.lock().clone();
    match &args {
        serde_json::Value::Object(o) => {
            let pubkey = if let Some(pk) = o.get("pubkey") {
                PublicKey::from_str(pk.as_str().ok_or_else(|| anyhow!("bad pubkey string"))?)
                    .context("invalid pubkey")?
            } else {
                return Err(anyhow!("no pubkey given"));
            };
            let their_funding_msat = if let Some(msats) = o.get("their_funding_sat") {
                msats
                    .as_u64()
                    .ok_or_else(|| anyhow!("their_funding_sat: not a valid number"))?
                    * 1000
            } else {
                return Err(anyhow!("their_funding_sat not set"));
            };
            let public = if let Some(p) = o.get("public") {
                p.as_bool()
                    .ok_or_else(|| anyhow!("public: not a valid boolean"))?
            } else {
                return Err(anyhow!("public not set"));
            };

            if let Some(r) = o.get("rule") {
                let rule = r
                    .as_str()
                    .ok_or_else(|| anyhow!("rule: not a valid string"))?;
                let data = collect_data(
                    &plugin,
                    pubkey,
                    Amount::from_msat(their_funding_msat),
                    ChannelFlags { public },
                    rule,
                    config.ping_length,
                )
                .await?;
                let parse_result = parse_rule(rule)?;
                let parser = ClnrodParser::new();
                let (evaluate_result, reject_reason) = evaluate_rule(&parser, parse_result, &data)?;
                let reject_reason = if let Some(rej_res) = reject_reason {
                    rej_res
                } else {
                    "None".to_string()
                };

                let config = plugin.state().config.lock().clone();
                if config.send_mail {
                    notify(
                        &plugin,
                        "Clnrod TEST RULE",
                        &format!(
                            "Called clnrod-testrule, custom_rule_result: {evaluate_result}. \
                        Offending comparisons: {reject_reason}"
                        ),
                        Some(pubkey),
                        NotifyVerbosity::Error,
                    )
                    .await;
                }

                Ok(json!({"custom_rule_result":evaluate_result, "reject_reason":reject_reason}))
            } else {
                Err(anyhow!(
                    "Invalid input! Use command like this: lightning-cli clnrod-testparse \
                    rule='x == 5' pubkey=XXXXX their_funding_sat=50000 public=true"
                ))
            }
        }
        _ => Err(anyhow!(
            "Invalid input! Use command like this: lightning-cli clnrod-testparse rule='x == 5' \
            pubkey=XXXXX their_funding_sat=50000 public=true"
        )),
    }
}

pub async fn clnrod_testmail(
    plugin: Plugin<PluginState>,
    _args: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let config = plugin.state().config.lock().clone();
    if config.send_mail {
        notify(
            &plugin,
            "Clnrod TEST EMAIL",
            "called clnrod-testmail",
            None,
            NotifyVerbosity::Error,
        )
        .await;
        return Ok(json!({"result":"success"}));
    }
    Ok(json!({"result":"not configured"}))
}

pub async fn clnrod_testping(
    plugin: Plugin<PluginState>,
    args: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let (pubkey_str, count, length) = match &args {
        serde_json::Value::Object(o) => {
            let pubkey_str = if let Some(pk) = o.get("pubkey") {
                pk.as_str().ok_or_else(|| anyhow!("bad pubkey string"))?
            } else {
                return Err(anyhow!("no pubkey given"));
            };
            let count = if let Some(pk) = o.get("count") {
                pk.as_u64().ok_or_else(|| anyhow!("bad count number"))?
            } else {
                3
            };
            let length = if let Some(pk) = o.get("length") {
                u16::try_from(pk.as_u64().ok_or_else(|| anyhow!("bad length number"))?)?
            } else {
                plugin.state().config.lock().ping_length
            };
            (pubkey_str, count, length)
        }
        serde_json::Value::Array(a) => {
            if a.len() != 1 && a.len() != 2 && a.len() != 3 {
                return Err(anyhow!(
                    "Invalid amount of arguments! \
                Only provide the pubkey of the node you want to ping and optionally \
                how often you want to ping."
                ));
            } else {
                let pubkey_str = a
                    .first()
                    .unwrap()
                    .as_str()
                    .ok_or_else(|| anyhow!("bad pubkey string"))?;
                let count = if let Some(c) = a.get(1) {
                    c.as_u64().ok_or_else(|| anyhow!("bad count number"))?
                } else {
                    3
                };
                let length = if let Some(c) = a.get(2) {
                    u16::try_from(c.as_u64().ok_or_else(|| anyhow!("bad length number"))?)
                        .context("length out of valid range")?
                } else {
                    plugin.state().config.lock().ping_length
                };
                (pubkey_str, count, length)
            }
        }
        _ => {
            return Err(anyhow!(
            "Invalid input! Use command like this: `lightning-cli clnrod-ping -k pubkey=<pubkey> \
            [count=<count>] [length=<length>]` or this: \
            `lightning-cli clnrod-ping <pubkey> [<count>] [<length>]`"
        ))
        }
    };
    if count < 1 {
        return Err(anyhow!("count must be >=1"));
    }
    if length < 1 {
        return Err(anyhow!("length must be >=1"));
    }
    let pubkey = PublicKey::from_str(pubkey_str).context("invalid pubkey")?;

    let rpc_path =
        Path::new(&plugin.configuration().lightning_dir).join(plugin.configuration().rpc_file);
    let mut rpc = ClnRpc::new(rpc_path).await?;

    rpc.call_typed(&ConnectRequest {
        host: None,
        port: None,
        id: pubkey.to_string(),
    })
    .await?;

    let pings = ln_ping(plugin, pubkey, count, length).await?;
    let sum_pings = pings.iter().map(|y| *y as u64).sum::<u64>();
    let median = pings.iter().nth(pings.len() / 2).unwrap_or(&0);
    Ok(json!({"min":pings.iter().min(),
        "avg":sum_pings/(pings.len() as u64),
        "median":median,
        "max":pings.iter().max()}))
}
