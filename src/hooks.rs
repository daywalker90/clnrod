use anyhow::Error;
use cln_plugin::Plugin;
use cln_rpc::{
    hooks::{
        actions::{Openchannel2Action, Openchannel2Result, OpenchannelAction, OpenchannelResult},
        events::{Openchannel2Event, OpenchannelEvent},
    },
    primitives::{Amount, PublicKey},
};

use crate::{
    collect::collect_data,
    notify::notify,
    parser::{evaluate_rule, parse_rule},
    structs::{BlockMode, ChannelFlags, ClnrodParser, Config, NotifyVerbosity, PluginState},
};

pub async fn openchannel_hook(
    plugin: Plugin<PluginState>,
    event: OpenchannelEvent,
) -> Result<OpenchannelAction, Error> {
    match release_hook(
        plugin.clone(),
        event.openchannel.id,
        event.openchannel.funding_msat,
        parse_channel_flags(event.openchannel.channel_flags),
    )
    .await
    {
        Ok(is_zeroconf_allowed) => {
            let zeroconf_channel = if let Some(ct) = event.openchannel.channel_type {
                ct.bits.contains(&50)
            } else {
                false
            };

            let mindepth = if zeroconf_channel && is_zeroconf_allowed {
                Some(0)
            } else {
                None
            };

            Ok(OpenchannelAction {
                close_to: None,
                error_message: None,
                mindepth,
                reserve: None,
                result: OpenchannelResult::CONTINUE,
            })
        }
        Err(e) => Ok(OpenchannelAction {
            close_to: None,
            error_message: Some(e),
            mindepth: None,
            reserve: None,
            result: OpenchannelResult::REJECT,
        }),
    }
}

pub async fn openchannel2_hook(
    plugin: Plugin<PluginState>,
    event: Openchannel2Event,
) -> Result<Openchannel2Action, Error> {
    match release_hook(
        plugin.clone(),
        event.openchannel2.id,
        event.openchannel2.their_funding_msat,
        parse_channel_flags(event.openchannel2.channel_flags),
    )
    .await
    {
        Ok(_o) => Ok(Openchannel2Action {
            close_to: None,
            error_message: None,
            result: Openchannel2Result::CONTINUE,
            our_funding_msat: None,
            psbt: None,
        }),
        Err(e) => Ok(Openchannel2Action {
            close_to: None,
            error_message: Some(e),
            result: Openchannel2Result::REJECT,
            our_funding_msat: None,
            psbt: None,
        }),
    }
}

async fn release_hook(
    plugin: Plugin<PluginState>,
    pubkey: PublicKey,
    their_funding_msat: Amount,
    channel_flags: ChannelFlags,
) -> Result<bool, String> {
    let pubkey_list = plugin.state().pubkey_list.lock().clone();
    let config = plugin.state().config.lock().clone();

    let list_matched = pubkey_list.contains(&pubkey);
    let is_zeroconf_allowed = plugin.state().zero_conf_list.lock().contains(&pubkey);
    log::debug!(
        "release_hook: start, list_matched:{list_matched},\
         is_zeroconf_allowed:{is_zeroconf_allowed}"
    );

    let allowed_custom = if !list_matched && !config.custom_rule.is_empty() {
        let data = match collect_data(
            &plugin,
            pubkey,
            their_funding_msat,
            channel_flags,
            &config.custom_rule,
            config.ping_length,
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
                return Err(create_reject_response(&config, "internal error"));
            }
        };
        let parser = ClnrodParser::new();
        match evaluate_rule(&parser, parse_rule(&config.custom_rule).unwrap(), &data) {
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
                return Err(create_reject_response(&config, "internal error"));
            }
        }
    } else {
        None
    };
    log::debug!("release_hook: done, allowed_custom:{allowed_custom:?}");
    match config.block_mode {
        BlockMode::Allow => {
            if list_matched {
                notify(
                    &plugin,
                    "Clnrod channel accepted.",
                    "On allowlist",
                    Some(pubkey),
                    NotifyVerbosity::Accepted,
                )
                .await;
                Ok(is_zeroconf_allowed)
            } else if let Some(cu) = allowed_custom {
                if cu.0 {
                    notify(
                        &plugin,
                        "Clnrod channel accepted.",
                        "Not on allowlist, but accepted by custom rule",
                        Some(pubkey),
                        NotifyVerbosity::Accepted,
                    )
                    .await;
                    Ok(is_zeroconf_allowed)
                } else {
                    let reject_reason = if let Some(rej_res) = cu.1 {
                        rej_res
                    } else {
                        "Reject reason not found".to_string()
                    };
                    notify(
                        &plugin,
                        "Clnrod channel rejected.",
                        &format!(
                            "Not on allowlist and not accepted by custom rule. \
                        Offending comparisons: `{reject_reason}`"
                        ),
                        Some(pubkey),
                        NotifyVerbosity::All,
                    )
                    .await;
                    Err(create_reject_response(&config, &reject_reason))
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
                Err(create_reject_response(&config, "not whitelisted"))
            }
        }
        BlockMode::Deny => {
            if list_matched {
                notify(
                    &plugin,
                    "Clnrod channel rejected.",
                    "On denylist",
                    Some(pubkey),
                    NotifyVerbosity::All,
                )
                .await;
                Err(create_reject_response(&config, "blacklisted"))
            } else if let Some(cu) = allowed_custom {
                if cu.0 {
                    notify(
                        &plugin,
                        "Clnrod channel accepted.",
                        "Not on denylist and accepted by custom rule",
                        Some(pubkey),
                        NotifyVerbosity::Accepted,
                    )
                    .await;
                    Ok(is_zeroconf_allowed)
                } else {
                    let reject_reason = if let Some(rej_res) = cu.1 {
                        rej_res
                    } else {
                        "Reject reason not found".to_string()
                    };
                    notify(
                        &plugin,
                        "Clnrod channel rejected.",
                        &format!(
                            "Not on denylist, but did not get accepted by custom rule. \
                        Offending comparisons: `{reject_reason}`"
                        ),
                        Some(pubkey),
                        NotifyVerbosity::All,
                    )
                    .await;
                    Err(create_reject_response(&config, &reject_reason))
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
                Ok(is_zeroconf_allowed)
            }
        }
    }
}

fn parse_channel_flags(channel_flags: u8) -> ChannelFlags {
    let lsb = channel_flags & 1;
    let public = lsb == 1;
    ChannelFlags { public }
}

fn create_reject_response(config: &Config, reason: &str) -> String {
    if config.leak_reason {
        format!("{} Reason: {}", config.deny_message, reason)
    } else {
        config.deny_message.clone()
    }
}
