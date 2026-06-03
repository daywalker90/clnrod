use std::{
    io,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use anyhow::{Context, Error, anyhow};
use cln_plugin::Plugin;
use cln_rpc::{
    ClnRpc,
    model::requests::ConnectRequest,
    primitives::{Amount, PublicKey},
};
use serde_json::json;
use tokio::{
    fs::{self},
    io::{AsyncReadExt, AsyncWriteExt},
    time,
};

use crate::{
    OPT_BLOCK_MODE,
    PLUGIN_NAME,
    collect::{collect_data, ln_ping},
    config::{read_pubkey_list, read_zeroconf_list},
    notify::notify,
    parser::{evaluate_rule, parse_rule},
    structs::{BlockMode, ChannelFlags, ClnrodParser, NotifyVerbosity, PluginState},
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
        read_pubkey_list(plugin.state().pubkey_list.clone(), &plugin_dir, block_mode).await?;

    let (zero_removed, zero_added) =
        read_zeroconf_list(plugin.state().zero_conf_list.clone(), &plugin_dir).await?;

    Ok(json!({"removed":removed, "added":added,
         "zeroconf_removed":zero_removed, "zeroconf_added":zero_added}))
}

pub async fn clnrod_testrule(
    plugin: Plugin<PluginState>,
    args: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let config = plugin.state().config.lock().clone();
    let (pubkey, public, their_funding_msat, rule) = match &args {
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

            let rule = if let Some(r) = o.get("rule") {
                r.as_str()
                    .ok_or_else(|| anyhow!("rule: not a valid string"))?
            } else {
                return Err(anyhow!(
                    "Invalid input! Use command like this: lightning-cli clnrod-testparse \
                    rule='x == 5' pubkey=XXXXX their_funding_sat=50000 public=true"
                ));
            };
            (pubkey, public, their_funding_msat, rule)
        }
        serde_json::Value::Array(a) => {
            let pubkey = if let Some(pk) = a.first() {
                PublicKey::from_str(pk.as_str().ok_or_else(|| anyhow!("bad pubkey string"))?)
                    .context("invalid pubkey")?
            } else {
                return Err(anyhow!("no pubkey given"));
            };
            let public = if let Some(p) = a.get(1) {
                p.as_bool()
                    .ok_or_else(|| anyhow!("public: not a valid boolean"))?
            } else {
                return Err(anyhow!("public not set"));
            };
            let their_funding_msat = if let Some(msats) = a.get(2) {
                msats
                    .as_u64()
                    .ok_or_else(|| anyhow!("their_funding_sat: not a valid number"))?
                    * 1000
            } else {
                return Err(anyhow!("their_funding_sat not set"));
            };
            let rule = if let Some(r) = a.get(3) {
                r.as_str()
                    .ok_or_else(|| anyhow!("rule: not a valid string"))?
            } else {
                return Err(anyhow!(
                    "Invalid input! Use command like this: lightning-cli clnrod-testparse \
                    rule='x == 5' pubkey=XXXXX their_funding_sat=50000 public=true"
                ));
            };
            (pubkey, public, their_funding_msat, rule)
        }
        _ => {
            return Err(anyhow!(
                "Invalid input! Use command like this: lightning-cli clnrod-testparse rule='x == 5' \
            pubkey=XXXXX their_funding_sat=50000 public=true"
            ));
        }
    };
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
            }
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
        _ => {
            return Err(anyhow!(
                "Invalid input! Use command like this: `lightning-cli clnrod-ping -k pubkey=<pubkey> \
            [count=<count>] [length=<length>]` or this: \
            `lightning-cli clnrod-ping <pubkey> [<count>] [<length>]`"
            ));
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
    let sum_pings = pings.iter().map(|y| u64::from(*y)).sum::<u64>();
    let median = pings.get(pings.len() / 2).unwrap_or(&0);
    Ok(json!({"min":pings.iter().min(),
        "avg":sum_pings/(pings.len() as u64),
        "median":median,
        "max":pings.iter().max()}))
}

pub async fn clnrod_managelists(
    plugin: Plugin<PluginState>,
    args: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let (listtype_str, operation_str, pubkey_str) = parse_managelists_args(&args)?;
    let pubkey = PublicKey::from_str(pubkey_str).context("invalid pubkey")?;

    match listtype_str {
        "allow" => {
            if plugin.state().config.lock().block_mode == BlockMode::Deny {
                return Err(anyhow!("You are configured to use the denylist!"));
            }
        }
        "deny" => {
            if plugin.state().config.lock().block_mode == BlockMode::Allow {
                return Err(anyhow!("You are configured to use the allowlist!"));
            }
        }
        _ => (),
    }

    {
        let pubkey_list = match listtype_str {
            "allow" | "deny" => plugin.state().pubkey_list.lock(),
            "zeroconf" => plugin.state().zero_conf_list.lock(),
            _ => return Err(anyhow!("listtype must be `allow`, `deny` or `zeroconf`")),
        };
        match operation_str {
            "add" => {
                if pubkey_list.contains(&pubkey) {
                    return Err(anyhow!("pubkey already in list"));
                }
            }
            "remove" => {
                if !pubkey_list.contains(&pubkey) {
                    return Err(anyhow!("pubkey not found in list"));
                }
            }
            _ => return Err(anyhow!("operation must be `add` or `remove`")),
        }
    }

    let clnrod_path = PathBuf::from_str(&plugin.configuration().lightning_dir)?.join(PLUGIN_NAME);

    match operation_str {
        "add" => {
            add_line(
                clnrod_path.join(format!("{listtype_str}list.txt")),
                pubkey_str,
            )
            .await?;
        }
        "remove" => {
            remove_line(
                clnrod_path.join(format!("{listtype_str}list.txt")),
                pubkey_str,
            )
            .await?;
        }
        _ => return Err(anyhow!("operation must be `add` or `remove`")),
    }

    {
        let mut pubkey_list = match listtype_str {
            "allow" | "deny" => plugin.state().pubkey_list.lock(),
            "zeroconf" => plugin.state().zero_conf_list.lock(),
            _ => return Err(anyhow!("listtype must be `allow`, `deny` or `zeroconf`")),
        };
        match operation_str {
            "add" => {
                pubkey_list.insert(pubkey);
            }
            "remove" => {
                pubkey_list.remove(&pubkey);
            }
            _ => return Err(anyhow!("operation must be `add` or `remove`")),
        }
    }

    Ok(json!({"result":"success"}))
}

fn parse_managelists_args(args: &serde_json::Value) -> Result<(&str, &str, &str), Error> {
    let (listtype_val, operation_val, pubkey_val) = match args {
        serde_json::Value::Array(values) => (
            values
                .first()
                .ok_or_else(|| anyhow!("listtype not found"))?,
            values
                .get(1)
                .ok_or_else(|| anyhow!("operation not found"))?,
            values.get(2).ok_or_else(|| anyhow!("pubkey not found"))?,
        ),
        serde_json::Value::Object(map) => (
            map.get("listtype")
                .ok_or_else(|| anyhow!("listtype not found"))?,
            map.get("operation")
                .ok_or_else(|| anyhow!("operation not found"))?,
            map.get("pubkey")
                .ok_or_else(|| anyhow!("pubkey not found"))?,
        ),
        _ => return Err(anyhow!("Expected array or object")),
    };

    let listtype_str = listtype_val
        .as_str()
        .ok_or_else(|| anyhow!("listtype must be a string"))?;
    let operation_str = operation_val
        .as_str()
        .ok_or_else(|| anyhow!("operation must be a string"))?;
    let pubkey_str = pubkey_val
        .as_str()
        .ok_or_else(|| anyhow!("pubkey must be a string"))?;
    Ok((listtype_str, operation_str, pubkey_str))
}

struct FileLock {
    lock_path: PathBuf,
}

impl FileLock {
    async fn acquire(target: &Path) -> Result<Self, Error> {
        let lock_path = PathBuf::from(format!("{}.lock", target.display()));

        loop {
            let result = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
                .await;

            match result {
                Ok(_) => {
                    return Ok(Self { lock_path });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    time::sleep(Duration::from_millis(50)).await;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

pub async fn add_line(path: impl AsRef<Path>, line: &str) -> Result<(), Error> {
    let path = path.as_ref();

    let _lock = FileLock::acquire(path).await?;

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;

    file.write_all(line.as_bytes()).await?;
    file.write_all(b"\n").await?;
    file.flush().await?;

    Ok(())
}

pub async fn remove_line(path: impl AsRef<Path>, line: &str) -> Result<(), Error> {
    let path = path.as_ref();

    let _lock = FileLock::acquire(path).await?;

    let mut content = String::new();

    {
        let mut file = fs::OpenOptions::new().read(true).open(path).await?;
        file.read_to_string(&mut content).await?;
    }

    let filtered = content
        .lines()
        .filter(|l| *l != line)
        .collect::<Vec<_>>()
        .join("\n");

    let tmp_path = PathBuf::from(format!("{}.tmp", path.display()));

    {
        let mut tmp = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)
            .await?;

        tmp.write_all(filtered.as_bytes()).await?;

        if !filtered.is_empty() {
            tmp.write_all(b"\n").await?;
        }

        tmp.flush().await?;
    }

    // Atomic replacement on Unix.
    // On Windows you may need to remove the destination first.
    #[cfg(windows)]
    {
        let _ = fs::remove_file(path).await;
    }

    fs::rename(&tmp_path, path).await?;

    Ok(())
}
