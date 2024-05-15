use std::str::FromStr;

use crate::{
    collect::collect_data,
    notify::notify,
    parser::{evaluate_rule, parse_rule},
    structs::{BlockMode, ChannelFlags, Config, NotifyVerbosity, PluginState},
};
use anyhow::{anyhow, Error};
use cln_plugin::Plugin;
use cln_rpc::primitives::{Amount, PublicKey};
use log::debug;
use serde_json::json;

pub async fn openchannel_hook(
    plugin: Plugin<PluginState>,
    v: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let config = plugin.state().config.lock().clone();
    let (pubkey, their_funding_msat, channel_flags) = match parse_openchannel(&v) {
        Ok(parsed) => parsed,
        Err(e) => {
            notify(
                &plugin,
                "Clnrod channel rejected. V1 HOOK PARSING ERROR",
                &format!("Error:\n{}\nHook input:\n{}", e, v),
                None,
                NotifyVerbosity::Error,
            )
            .await;
            return Ok(create_reject_response(&config));
        }
    };
    Ok(release_hook(plugin.clone(), pubkey, their_funding_msat, channel_flags).await)
}

fn parse_openchannel(v: &serde_json::Value) -> Result<(PublicKey, Amount, ChannelFlags), Error> {
    let openchannel = v
        .get("openchannel")
        .ok_or_else(|| anyhow!("Missing 'openchannel' field"))?;
    let id = openchannel
        .get("id")
        .and_then(|id| id.as_str())
        .ok_or_else(|| anyhow!("Missing 'id' field"))?;
    let pubkey = PublicKey::from_str(id)?;
    let their_funding_msat = Amount::from_msat(
        openchannel
            .get("funding_msat")
            .ok_or_else(|| anyhow!("Missing 'funding_msat' field"))?
            .as_u64()
            .ok_or_else(|| anyhow!("'funding_msat' field is not a u64"))?,
    );
    let channel_flags = parse_channel_flags(
        openchannel
            .get("channel_flags")
            .ok_or_else(|| anyhow!("Missing 'channel_flags' field"))?
            .as_u64()
            .ok_or_else(|| anyhow!("'channel_flags' field is not a u64"))?,
    )?;

    Ok((pubkey, their_funding_msat, channel_flags))
}

pub async fn openchannel2_hook(
    plugin: Plugin<PluginState>,
    v: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let config = plugin.state().config.lock().clone();
    let (pubkey, their_funding_msat, channel_flags) = match parse_openchannel2(&v) {
        Ok(parsed) => parsed,
        Err(e) => {
            notify(
                &plugin,
                "Clnrod channel rejected. V2 HOOK PARSING ERROR",
                &format!("Error:\n{}\nHook input:\n{}", e, v),
                None,
                NotifyVerbosity::Error,
            )
            .await;
            return Ok(create_reject_response(&config));
        }
    };
    Ok(release_hook(plugin.clone(), pubkey, their_funding_msat, channel_flags).await)
}

fn parse_openchannel2(v: &serde_json::Value) -> Result<(PublicKey, Amount, ChannelFlags), Error> {
    let openchannel2 = v
        .get("openchannel2")
        .ok_or_else(|| anyhow!("Missing 'openchannel2' field"))?;
    let id = openchannel2
        .get("id")
        .and_then(|id| id.as_str())
        .ok_or_else(|| anyhow!("Missing 'id' field"))?;
    let pubkey = PublicKey::from_str(id)?;
    let their_funding_msat = Amount::from_msat(
        openchannel2
            .get("their_funding_msat")
            .ok_or_else(|| anyhow!("Missing 'their_funding_msat' field"))?
            .as_u64()
            .ok_or_else(|| anyhow!("'their_funding_msat' field is not a u64"))?,
    );
    let channel_flags = parse_channel_flags(
        openchannel2
            .get("channel_flags")
            .ok_or_else(|| anyhow!("Missing 'channel_flags' field"))?
            .as_u64()
            .ok_or_else(|| anyhow!("'channel_flags' field is not a u64"))?,
    )?;

    Ok((pubkey, their_funding_msat, channel_flags))
}

async fn release_hook(
    plugin: Plugin<PluginState>,
    pubkey: PublicKey,
    their_funding_msat: Amount,
    channel_flags: ChannelFlags,
) -> serde_json::Value {
    let pubkey_list = plugin.state().pubkey_list.lock().clone();
    let config = plugin.state().config.lock().clone();

    let list_matched = pubkey_list.contains(&pubkey);
    debug!("release_hook: start, {}", list_matched);

    let allowed_custom = if !list_matched && !config.custom_rule.value.is_empty() {
        let data = match collect_data(
            &plugin,
            pubkey,
            their_funding_msat,
            channel_flags,
            &config.custom_rule.value,
        )
        .await
        {
            Ok(da) => da,
            Err(e) => {
                notify(
                    &plugin,
                    "Clnrod channel rejected. COLLECT_DATA ERROR",
                    &e.to_string(),
                    Some(pubkey),
                    NotifyVerbosity::Error,
                )
                .await;
                return create_reject_response(&config);
            }
        };
        match evaluate_rule(parse_rule(&config.custom_rule.value).unwrap(), &data) {
            Ok(o) => Some(o),
            Err(e) => {
                notify(
                    &plugin,
                    "Clnrod channel rejected. EVALUATE_RULE ERROR",
                    &e.to_string(),
                    Some(pubkey),
                    NotifyVerbosity::Error,
                )
                .await;
                return create_reject_response(&config);
            }
        }
    } else {
        None
    };
    debug!("release_hook: done, {} {:?}", list_matched, allowed_custom);
    match config.block_mode.value {
        BlockMode::Allow => {
            if list_matched {
                notify(
                    &plugin,
                    "Clnrod channel accepted.",
                    "on allowlist",
                    Some(pubkey),
                    NotifyVerbosity::Accepted,
                )
                .await;
                json!({"result":"continue"})
            } else if let Some(cu) = allowed_custom {
                if cu {
                    notify(
                        &plugin,
                        "Clnrod channel accepted.",
                        "not on allowlist, but accepted by custom rule",
                        Some(pubkey),
                        NotifyVerbosity::Accepted,
                    )
                    .await;
                    json!({"result":"continue"})
                } else {
                    notify(
                        &plugin,
                        "Clnrod channel rejected.",
                        "not on allowlist and not accepted by custom rule",
                        Some(pubkey),
                        NotifyVerbosity::All,
                    )
                    .await;
                    create_reject_response(&config)
                }
            } else {
                notify(
                    &plugin,
                    "Clnrod channel rejected.",
                    "Not on allowlist and no custom rule",
                    Some(pubkey),
                    NotifyVerbosity::All,
                )
                .await;
                create_reject_response(&config)
            }
        }
        BlockMode::Deny => {
            if list_matched {
                notify(
                    &plugin,
                    "Clnrod channel rejected.",
                    "on denylist",
                    Some(pubkey),
                    NotifyVerbosity::All,
                )
                .await;
                create_reject_response(&config)
            } else if let Some(cu) = allowed_custom {
                if cu {
                    notify(
                        &plugin,
                        "Clnrod channel accepted.",
                        "not on denylist and accepted by custom rule",
                        Some(pubkey),
                        NotifyVerbosity::Accepted,
                    )
                    .await;
                    json!({"result":"continue"})
                } else {
                    notify(
                        &plugin,
                        "Clnrod channel rejected.",
                        "not on denylist, but did not get accepted by custom rule",
                        Some(pubkey),
                        NotifyVerbosity::All,
                    )
                    .await;
                    create_reject_response(&config)
                }
            } else {
                notify(
                    &plugin,
                    "Clnrod channel accepted.",
                    "not on denylist and no custom rule",
                    Some(pubkey),
                    NotifyVerbosity::Accepted,
                )
                .await;
                json!({"result":"continue"})
            }
        }
    }
}

fn parse_channel_flags(channel_flags: u64) -> Result<ChannelFlags, Error> {
    let lsb = channel_flags & 1;
    let public = lsb == 1;
    Ok(ChannelFlags { public })
}

fn create_reject_response(config: &Config) -> serde_json::Value {
    if config.deny_message.value.is_empty() {
        json!({"result": "reject"})
    } else {
        json!({"result": "reject", "error_message": config.deny_message.value})
    }
}
