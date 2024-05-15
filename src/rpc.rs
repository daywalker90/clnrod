use std::{path::Path, str::FromStr};

use anyhow::{anyhow, Context, Error};
use cln_plugin::Plugin;
use cln_rpc::primitives::{Amount, PublicKey};
use serde_json::json;

use crate::{
    collect::collect_data,
    config::read_pubkey_list,
    notify::notify,
    parser::{evaluate_rule, parse_rule},
    structs::{BlockMode, ChannelFlags, NotifyVerbosity, PluginState},
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
                plugin.state().peerdata_cache.lock().remove(&pubkey);
                let data = collect_data(
                    &plugin,
                    pubkey,
                    Amount::from_msat(their_funding_msat),
                    ChannelFlags { public },
                    rule,
                )
                .await?;
                let parse_result = parse_rule(rule)?;
                let evaluate_result = evaluate_rule(parse_result, &data)?;
                Ok(json!({"parse_result":evaluate_result}))
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
            &config,
            "Clnrod TEST EMAIL",
            "called clnrod-testmail",
            "FAKEKEY".to_string(),
            NotifyVerbosity::Error,
        )
        .await;
        return Ok(json!({"result":"success"}));
    }
    Ok(json!({"result":"not configured"}))
}
