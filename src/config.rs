use anyhow::{anyhow, Error};
use cln_rpc::primitives::PublicKey;
use log::{info, warn};

use std::collections::HashSet;
use std::path::PathBuf;
use std::{path::Path, str::FromStr};
use tokio::fs::{self, File};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::parser::parse_rule;
use crate::structs::NotifyVerbosity;
use crate::PLUGIN_NAME;
use crate::{
    structs::{BlockMode, Config},
    PluginState,
};
// fn validate_u64_input(n: u64, var_name: &str, gteq: u64) -> Result<u64, Error> {
//     if n < gteq {
//         return Err(anyhow!(
//             "{} must be greater than or equal to {}",
//             var_name,
//             gteq
//         ));
//     }

//     Ok(n)
// }

// fn validate_i64_input(n: i64, var_name: &str, gteq: i64) -> Result<i64, Error> {
//     if n < gteq {
//         return Err(anyhow!(
//             "{} must be greater than or equal to {}",
//             var_name,
//             gteq
//         ));
//     }

//     Ok(n)
// }

// fn options_value_to_u64(opt: &ConfigOption<Integer>, value: i64, gteq: u64) -> Result<u64, Error> {
//     if value >= 0 {
//         validate_u64_input(value as u64, opt.name, gteq)
//     } else {
//         Err(anyhow!(
//             "{} needs to be a positive number and not `{}`.",
//             opt.name,
//             value
//         ))
//     }
// }

// fn value_to_u64(var_name: &str, value: &serde_json::Value, gteq: u64) -> Result<u64, Error> {
//     match value {
//         serde_json::Value::Number(b) => match b.as_u64() {
//             Some(n) => validate_u64_input(n, var_name, gteq),
//             None => Err(anyhow!(
//                 "Could not read a positive number for {}.",
//                 var_name
//             )),
//         },
//         _ => Err(anyhow!("{} must be a positive number.", var_name)),
//     }
// }

// fn value_to_i64(var_name: &str, value: &serde_json::Value, gteq: i64) -> Result<i64, Error> {
//     match value {
//         serde_json::Value::Number(b) => match b.as_i64() {
//             Some(n) => validate_i64_input(n, var_name, gteq),
//             None => Err(anyhow!("Could not read a number for {}.", var_name)),
//         },
//         _ => Err(anyhow!("{} must be a number.", var_name)),
//     }
// }

// fn str_to_u64(var_name: &str, value: &str, gteq: u64) -> Result<u64, Error> {
//     match value.parse::<u64>() {
//         Ok(n) => validate_u64_input(n, var_name, gteq),
//         Err(e) => Err(anyhow!(
//             "Could not parse a positive number from `{}` for {}: {}",
//             value,
//             var_name,
//             e
//         )),
//     }
// }

// fn str_to_i64(var_name: &str, value: &str, gteq: i64) -> Result<i64, Error> {
//     match value.parse::<i64>() {
//         Ok(n) => validate_i64_input(n, var_name, gteq),
//         Err(e) => Err(anyhow!(
//             "Could not parse a number from `{}` for {}: {}",
//             value,
//             var_name,
//             e
//         )),
//     }
// }

// pub fn validateargs(args: serde_json::Value, config: &mut Config) -> Result<(), Error> {
//     if let serde_json::Value::Object(i) = args {
//         for (key, value) in i.iter() {
//             match key {
//                 name if name.eq(&config.deny_message.name) => match value {
//                     serde_json::Value::String(b) => config.deny_message.value = b.clone(),
//                     _ => return Err(anyhow!("Not a string: {}", config.deny_message.name)),
//                 },
//                 other => return Err(anyhow!("option not found:{:?}", other)),
//             };
//         }
//     };
//     Ok(())
// }

async fn get_general_config_file(lightning_dir: &String) -> String {
    match fs::read_to_string(Path::new(lightning_dir).parent().unwrap().join("config")).await {
        Ok(file2) => file2,
        Err(_) => {
            warn!("No general config file found!");
            String::new()
        }
    }
}

async fn get_network_config_file(lightning_dir: &String) -> String {
    match fs::read_to_string(Path::new(lightning_dir).join("config")).await {
        Ok(file) => file,
        Err(_) => {
            warn!("No network config file found!");
            String::new()
        }
    }
}

pub async fn read_config(
    lightning_dir: String,
    state: &PluginState,
) -> Result<(usize, usize), Error> {
    info!("reading config...");
    let general_configfile = get_general_config_file(&lightning_dir).await;
    let network_configfile = get_network_config_file(&lightning_dir).await;

    let mut config = state.config.lock().clone();
    parse_config_file(general_configfile, &mut config)?;
    parse_config_file(network_configfile, &mut config)?;
    activate_mail(&mut config);

    let plugin_dir = Path::new(&lightning_dir).join(PLUGIN_NAME);
    let new_pubkey_list = read_pubkey_list(plugin_dir, &mut config).await?;

    if !config.custom_rule.value.is_empty() {
        parse_rule(&config.custom_rule.value)?;
    }

    let mut pubkey_list = state.pubkey_list.lock();

    let removed = pubkey_list.difference(&new_pubkey_list).count();
    info!("Reload: Removed {} peers", removed);

    let added = new_pubkey_list.difference(&pubkey_list).count();
    info!("Reload: Added {} peers", added);

    *pubkey_list = new_pubkey_list;

    *state.config.lock() = config;

    state.peerdata_cache.lock().clear();
    info!("cache cleared!");

    info!("config reloaded!");
    Ok((removed, added))
}

fn parse_config_file(configfile: String, config: &mut Config) -> Result<(), Error> {
    for line in configfile.lines() {
        if let Some(sl) = line.split_once('=') {
            let name = sl.0;
            let value = sl.1;

            match name {
                name if name.eq(config.block_mode.name) => {
                    config.block_mode.value = BlockMode::from_str(value)?
                }
                name if name.eq(config.deny_message.name) => {
                    config.deny_message.value = value.to_string()
                }
                name if name.eq(config.custom_rule.name) => {
                    parse_rule(value)?;
                    config.custom_rule.value = value.to_string()
                }
                name if name.eq(config.smtp_username.name) => {
                    config.smtp_username.value = value.to_string()
                }
                name if name.eq(config.smtp_password.name) => {
                    config.smtp_password.value = value.to_string()
                }
                name if name.eq(config.smtp_server.name) => {
                    config.smtp_server.value = value.to_string()
                }
                name if name.eq(config.smtp_port.name) => match value.parse::<u16>() {
                    Ok(n) => {
                        if n > 0 {
                            config.smtp_port.value = n
                        } else {
                            return Err(anyhow!(
                                "Error: Number needs to be greater than 0 for {}.",
                                config.smtp_port.name
                            ));
                        }
                    }
                    Err(e) => {
                        return Err(anyhow!(
                            "Error: Could not parse a positive number from `{}` for {}: {}",
                            value,
                            config.smtp_port.name,
                            e
                        ))
                    }
                },
                name if name.eq(config.email_from.name) => {
                    config.email_from.value = value.to_string()
                }
                name if name.eq(config.email_to.name) => config.email_to.value = value.to_string(),
                name if name.eq(config.notify_verbosity.name) => {
                    config.notify_verbosity.value = NotifyVerbosity::from_str(value)?
                }
                _ => (),
            }
        }
    }
    Ok(())
}

async fn read_pubkey_list(
    plugin_dir: PathBuf,
    config: &mut Config,
) -> Result<HashSet<PublicKey>, Error> {
    let file_path = match config.block_mode.value {
        BlockMode::Allow => plugin_dir.join("allowlist.txt"),
        BlockMode::Deny => plugin_dir.join("denylist.txt"),
    };

    fs::create_dir_all(&plugin_dir).await?;
    if !file_path.exists() {
        let _ = File::create(&file_path).await?;
    }

    let mut new_pubkey_list = HashSet::new();
    let block_file = File::open(file_path).await?;
    let file_reader = BufReader::new(block_file);
    let mut file_lines = file_reader.lines();
    while let Some(line) = file_lines.next_line().await? {
        new_pubkey_list.insert(PublicKey::from_str(&line)?);
    }
    Ok(new_pubkey_list)
}

// pub fn get_startup_options(
//     plugin: &ConfiguredPlugin<PluginState, tokio::io::Stdin, tokio::io::Stdout>,
//     state: PluginState,
// ) -> Result<(), Error> {
//     let mut config = state.config.lock();
//     if let Some(bm) = plugin.option(&OPT_BLOCK_MODE)? {
//         config.block_mode.value = BlockMode::from_str(&bm)?
//     };
//     if let Some(dm) = plugin.option(&OPT_DENY_MESSAGE)? {
//         config.deny_message.value = dm.to_string()
//     };
//     if let Some(cr) = plugin.option(&OPT_CUSTOM_RULE)? {
//         debug!("Config rule loaded: {}", cr.to_string());
//         config.custom_rule.value = cr.to_string()
//     };
//     if let Some(smtp_user) = plugin.option(&OPT_SMTP_USERNAME)? {
//         config.smtp_username.value = smtp_user.to_string()
//     };
//     if let Some(smtp_pw) = plugin.option(&OPT_SMTP_PASSWORD)? {
//         config.smtp_password.value = smtp_pw.to_string()
//     };
//     if let Some(smtp_server) = plugin.option(&OPT_SMTP_SERVER)? {
//         config.smtp_server.value = smtp_server.to_string()
//     };
//     if let Some(smtp_port) = plugin.option(&OPT_SMTP_PORT)? {
//         if smtp_port > 0 && smtp_port <= 65535 {
//             config.smtp_port.value = smtp_port as u16
//         } else {
//             return Err(anyhow!(
//                 "Error: Number needs to be >0 and <=65535 for {}.",
//                 config.smtp_port.name
//             ));
//         }
//     };
//     if let Some(email_from) = plugin.option(&OPT_EMAIL_FROM)? {
//         config.email_from.value = email_from.to_string()
//     };
//     if let Some(email_to) = plugin.option(&OPT_EMAIL_TO)? {
//         config.email_to.value = email_to.to_string()
//     };
//     if let Some(nv) = plugin.option(&OPT_NOTIFY_VERBOSITY)? {
//         config.notify_verbosity.value = NotifyVerbosity::from_str(&nv)?
//     };

//     activate_mail(&mut config);
//     Ok(())
// }

fn activate_mail(config: &mut Config) {
    if !config.smtp_username.value.is_empty()
        && !config.smtp_password.value.is_empty()
        && !config.smtp_server.value.is_empty()
        && config.smtp_port.value > 0
        && !config.email_from.value.is_empty()
        && !config.email_to.value.is_empty()
    {
        info!("Will try to send notifications via email");
        config.send_mail = true;
    } else {
        info!("Insufficient config for email notifications. Will not send emails")
    }
}
