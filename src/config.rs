use anyhow::{anyhow, Context, Error};
use cln_plugin::{options, ConfiguredPlugin, Plugin};
use cln_rpc::primitives::PublicKey;
use cln_rpc::RpcError;
use parking_lot::Mutex;
use serde_json::json;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::{path::Path, str::FromStr};
use tokio::fs::{self, File};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::parser::parse_rule;
use crate::structs::NotifyVerbosity;
use crate::{
    structs::{BlockMode, Config},
    PluginState,
};
use crate::{
    OPT_BLOCK_MODE, OPT_CUSTOM_RULE, OPT_DENY_MESSAGE, OPT_EMAIL_FROM, OPT_EMAIL_TO,
    OPT_LEAK_REASON, OPT_NOTIFY_VERBOSITY, OPT_PING_LENGTH, OPT_SMTP_PASSWORD, OPT_SMTP_PORT,
    OPT_SMTP_SERVER, OPT_SMTP_USERNAME, PLUGIN_NAME,
};

pub async fn read_config(
    lightning_dir: String,
    plugin: &ConfiguredPlugin<PluginState, tokio::io::Stdin, tokio::io::Stdout>,
    state: &PluginState,
) -> Result<(), Error> {
    get_startup_options(plugin, state)?;

    let plugin_dir = Path::new(&lightning_dir).join(PLUGIN_NAME);
    let block_mode = BlockMode::from_str(
        plugin
            .option_str(OPT_BLOCK_MODE)
            .unwrap()
            .unwrap()
            .as_str()
            .unwrap(),
    )?;

    read_pubkey_list(state.pubkey_list.clone(), plugin_dir, block_mode).await?;

    let mut config = state.config.lock();
    activate_mail(&mut config);

    Ok(())
}

pub async fn read_pubkey_list(
    pubkey_list: Arc<Mutex<HashSet<PublicKey>>>,
    plugin_dir: PathBuf,
    block_mode: BlockMode,
) -> Result<(usize, usize), Error> {
    let file_path = match block_mode {
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

    let mut pubkey_list = pubkey_list.lock();

    let removed = pubkey_list.difference(&new_pubkey_list).count();
    log::info!("Reload: Removed {} peers", removed);

    let added = new_pubkey_list.difference(&pubkey_list).count();
    log::info!("Reload: Added {} peers", added);

    *pubkey_list = new_pubkey_list;
    Ok((removed, added))
}

fn get_startup_options(
    plugin: &ConfiguredPlugin<PluginState, tokio::io::Stdin, tokio::io::Stdout>,
    state: &PluginState,
) -> Result<(), Error> {
    let mut config = state.config.lock();
    if let Some(bm) = plugin.option_str(OPT_BLOCK_MODE)? {
        check_option(&mut config, OPT_BLOCK_MODE, &bm)?;
    };
    if let Some(dm) = plugin.option_str(OPT_DENY_MESSAGE)? {
        check_option(&mut config, OPT_DENY_MESSAGE, &dm)?;
    };
    if let Some(lr) = plugin.option_str(OPT_LEAK_REASON)? {
        check_option(&mut config, OPT_LEAK_REASON, &lr)?;
    };
    if let Some(cr) = plugin.option_str(OPT_CUSTOM_RULE)? {
        check_option(&mut config, OPT_CUSTOM_RULE, &cr)?;
    };
    if let Some(pl) = plugin.option_str(OPT_PING_LENGTH)? {
        check_option(&mut config, OPT_PING_LENGTH, &pl)?;
    };
    if let Some(smtp_user) = plugin.option_str(OPT_SMTP_USERNAME)? {
        check_option(&mut config, OPT_SMTP_USERNAME, &smtp_user)?;
    };
    if let Some(smtp_pw) = plugin.option_str(OPT_SMTP_PASSWORD)? {
        check_option(&mut config, OPT_SMTP_PASSWORD, &smtp_pw)?;
    };
    if let Some(smtp_server) = plugin.option_str(OPT_SMTP_SERVER)? {
        check_option(&mut config, OPT_SMTP_SERVER, &smtp_server)?;
    };
    if let Some(smtp_port) = plugin.option_str(OPT_SMTP_PORT)? {
        check_option(&mut config, OPT_SMTP_PORT, &smtp_port)?;
    };
    if let Some(email_from) = plugin.option_str(OPT_EMAIL_FROM)? {
        check_option(&mut config, OPT_EMAIL_FROM, &email_from)?;
    };
    if let Some(email_to) = plugin.option_str(OPT_EMAIL_TO)? {
        check_option(&mut config, OPT_EMAIL_TO, &email_to)?;
    };
    if let Some(nv) = plugin.option_str(OPT_NOTIFY_VERBOSITY)? {
        check_option(&mut config, OPT_NOTIFY_VERBOSITY, &nv)?;
    };

    log::info!("all options valid!");

    Ok(())
}

fn check_option(config: &mut Config, name: &str, value: &options::Value) -> Result<(), Error> {
    match name {
        n if n.eq(OPT_BLOCK_MODE) => {
            config.block_mode = BlockMode::from_str(value.as_str().unwrap())?;
        }
        n if n.eq(OPT_DENY_MESSAGE) => {
            config.deny_message = value.as_str().unwrap().to_string();
        }
        n if n.eq(OPT_LEAK_REASON) => {
            config.leak_reason = match value {
                options::Value::String(s) => s.parse()?,
                options::Value::Boolean(b) => *b,
                _ => return Err(anyhow!("{} must be a boolean", OPT_LEAK_REASON)),
            }
        }
        n if n.eq(OPT_CUSTOM_RULE) => {
            parse_rule(value.as_str().unwrap())?;
            config.custom_rule = value.as_str().unwrap().to_string();
        }
        n if n.eq(OPT_PING_LENGTH) => {
            let ping_length = u16::try_from(value.as_i64().unwrap())
                .context(format!("{} out of valid range", OPT_PING_LENGTH))?;
            if ping_length == 0 {
                return Err(anyhow!("{} must be greater than 0", OPT_PING_LENGTH));
            }
            config.ping_length = ping_length;
        }
        n if n.eq(OPT_SMTP_USERNAME) => config.smtp_username = value.as_str().unwrap().to_string(),
        n if n.eq(OPT_SMTP_PASSWORD) => config.smtp_password = value.as_str().unwrap().to_string(),
        n if n.eq(OPT_SMTP_SERVER) => config.smtp_server = value.as_str().unwrap().to_string(),
        n if n.eq(OPT_SMTP_PORT) => {
            config.smtp_port = u16::try_from(value.as_i64().unwrap())
                .context(format!("{} out of valid range", OPT_SMTP_PORT))?
        }
        n if n.eq(OPT_EMAIL_FROM) => config.email_from = value.as_str().unwrap().to_string(),
        n if n.eq(OPT_EMAIL_TO) => config.email_to = value.as_str().unwrap().to_string(),
        n if n.eq(OPT_NOTIFY_VERBOSITY) => {
            config.notify_verbosity = NotifyVerbosity::from_str(value.as_str().unwrap())?;
        }
        _ => return Err(anyhow!("Unknown option: {}", name)),
    }
    Ok(())
}

fn parse_option(name: &str, value: &serde_json::Value) -> Result<options::Value, Error> {
    match name {
        n if n.eq(OPT_BLOCK_MODE) => {
            if let Some(bm_str) = value.as_str() {
                BlockMode::from_str(bm_str)?;
                Ok(options::Value::String(bm_str.to_string()))
            } else {
                Err(anyhow!("{} is not a string!", OPT_BLOCK_MODE))
            }
        }
        n if n.eq(OPT_NOTIFY_VERBOSITY) => {
            if let Some(nv_str) = value.as_str() {
                NotifyVerbosity::from_str(nv_str)?;
                Ok(options::Value::String(nv_str.to_string()))
            } else {
                Err(anyhow!("{} is not a string!", OPT_NOTIFY_VERBOSITY))
            }
        }
        n if n.eq(OPT_CUSTOM_RULE) => {
            if let Some(cr_str) = value.as_str() {
                parse_rule(cr_str)?;
                Ok(options::Value::String(cr_str.to_string()))
            } else {
                Err(anyhow!("{} is not a string!", OPT_CUSTOM_RULE))
            }
        }
        n if n.eq(OPT_SMTP_PORT) | n.eq(OPT_PING_LENGTH) => {
            if let Some(n_i64) = value.as_i64() {
                return Ok(options::Value::Integer(n_i64));
            } else if let Some(n_str) = value.as_str() {
                if let Ok(n_neg_i64) = n_str.parse::<i64>() {
                    return Ok(options::Value::Integer(n_neg_i64));
                }
            }
            Err(anyhow!("{} is not a valid integer!", name))
        }
        n if n.eq(OPT_LEAK_REASON) => match value {
            serde_json::Value::String(s) => Ok(options::Value::Boolean(s.parse()?)),
            serde_json::Value::Bool(b) => Ok(options::Value::Boolean(*b)),
            _ => return Err(anyhow!("{} must be a boolean", name)),
        },
        _ => {
            if value.is_string() {
                Ok(options::Value::String(value.as_str().unwrap().to_string()))
            } else {
                Err(anyhow!("{} is not a string!", name))
            }
        }
    }
}

pub async fn setconfig_callback(
    plugin: Plugin<PluginState>,
    args: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let name = args
        .get("config")
        .ok_or_else(|| anyhow!("Bad CLN object. No option name found!"))?
        .as_str()
        .ok_or_else(|| anyhow!("Bad CLN object. Option name not a string!"))?;
    let value = args
        .get("val")
        .ok_or_else(|| anyhow!("Bad CLN object. No value found for option: {name}"))?;

    let opt_value = parse_option(name, value).map_err(|e| {
        anyhow!(json!(RpcError {
            code: Some(-32602),
            message: e.to_string(),
            data: None
        }))
    })?;

    let mut config = plugin.state().config.lock();
    check_option(&mut config, name, &opt_value).map_err(|e| {
        anyhow!(json!(RpcError {
            code: Some(-32602),
            message: e.to_string(),
            data: None
        }))
    })?;

    plugin.set_option_str(name, opt_value).map_err(|e| {
        anyhow!(json!(RpcError {
            code: Some(-32602),
            message: e.to_string(),
            data: None
        }))
    })?;

    activate_mail(&mut config);

    if name.eq(OPT_CUSTOM_RULE) {
        plugin.state().peerdata_cache.lock().clear();
    }

    Ok(json!({}))
}

fn activate_mail(config: &mut Config) {
    if !config.smtp_username.is_empty()
        && !config.smtp_password.is_empty()
        && !config.smtp_server.is_empty()
        && config.smtp_port > 0
        && !config.email_from.is_empty()
        && !config.email_to.is_empty()
    {
        log::info!("Will try to send notifications via email");
        config.send_mail = true;
    } else {
        log::info!("Insufficient config for email notifications. Will not send emails");
    }
}
