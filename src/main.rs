use std::time::Duration;

use anyhow::anyhow;
use cln_plugin::{
    options::{ConfigOption, DefaultStringConfigOption, IntegerConfigOption, StringConfigOption},
    Builder,
};
use config::{read_config, setconfig_callback};
use hooks::{openchannel2_hook, openchannel_hook};
use log::info;
use pest_derive::Parser;
use rpc::{clnrod_reload, clnrod_testmail, clnrod_testping, clnrod_testrule};
use structs::PluginState;
use tokio::time;

mod collect;
mod config;
mod hooks;
mod notify;
mod parser;
mod rpc;
mod structs;
mod tasks;

pub const PLUGIN_NAME: &str = "clnrod";

const OPT_DENY_MESSAGE: &str = "clnrod-denymessage";
const OPT_BLOCK_MODE: &str = "clnrod-blockmode";
const OPT_CUSTOM_RULE: &str = "clnrod-customrule";
const OPT_PING_LENGTH: &str = "clnrod-pinglength";
const OPT_SMTP_USERNAME: &str = "clnrod-smtp-username";
const OPT_SMTP_PASSWORD: &str = "clnrod-smtp-password";
const OPT_SMTP_SERVER: &str = "clnrod-smtp-server";
const OPT_SMTP_PORT: &str = "clnrod-smtp-port";
const OPT_EMAIL_FROM: &str = "clnrod-email-from";
const OPT_EMAIL_TO: &str = "clnrod-email-to";
const OPT_NOTIFY_VERBOSITY: &str = "clnrod-notify-verbosity";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    std::env::set_var("CLN_PLUGIN_LOG", "clnrod=debug,info");
    log_panics::init();
    let state = PluginState::new();
    let opt_deny_message: StringConfigOption = ConfigOption::new_str_no_default(
        OPT_DENY_MESSAGE,
        "Message to send to a peer if clnrod denies a channel.",
    )
    .dynamic();
    let opt_block_mode: DefaultStringConfigOption = ConfigOption::new_str_with_default(
        OPT_BLOCK_MODE,
        "deny",
        "Use 'allow' list or 'deny' list.",
    )
    .dynamic();
    let opt_custom_rule: StringConfigOption =
        ConfigOption::new_str_no_default(OPT_CUSTOM_RULE, "Custom rule matching, see README.md")
            .dynamic();
    let opt_ping_length: IntegerConfigOption =
        ConfigOption::new_i64_no_default(OPT_PING_LENGTH, "Length of the ping messages in bytes")
            .dynamic();
    let opt_smtp_username: StringConfigOption =
        ConfigOption::new_str_no_default(OPT_SMTP_USERNAME, "Set smtp username").dynamic();
    let opt_smtp_password: StringConfigOption =
        ConfigOption::new_str_no_default(OPT_SMTP_PASSWORD, "Set smtp password").dynamic();
    let opt_smtp_server: StringConfigOption =
        ConfigOption::new_str_no_default(OPT_SMTP_SERVER, "Set smtp server").dynamic();
    let opt_smtp_port: IntegerConfigOption =
        ConfigOption::new_i64_no_default(OPT_SMTP_PORT, "Set smtp port").dynamic();
    let opt_email_from: StringConfigOption =
        ConfigOption::new_str_no_default(OPT_EMAIL_FROM, "Set email_from").dynamic();
    let opt_email_to: StringConfigOption =
        ConfigOption::new_str_no_default(OPT_EMAIL_TO, "Set email_to").dynamic();
    let opt_notify_verbosity: DefaultStringConfigOption = ConfigOption::new_str_with_default(
        OPT_NOTIFY_VERBOSITY,
        "all",
        "Set the verbosity level of notifications. One of: 'ERROR', 'ACCEPTED', 'ALL'",
    )
    .dynamic();

    let confplugin = match Builder::new(tokio::io::stdin(), tokio::io::stdout())
        .rpcmethod("clnrod-reload", "Reloads rules from file.", clnrod_reload)
        .rpcmethod("clnrod-testrule", "Test custom rule", clnrod_testrule)
        .rpcmethod("clnrod-testmail", "Test mail config", clnrod_testmail)
        .rpcmethod(
            "clnrod-testping",
            "Test the ping to a node",
            clnrod_testping,
        )
        .setconfig_callback(setconfig_callback)
        .option(opt_deny_message)
        .option(opt_block_mode)
        .option(opt_custom_rule)
        .option(opt_ping_length)
        .option(opt_smtp_username)
        .option(opt_smtp_password)
        .option(opt_smtp_server)
        .option(opt_smtp_port)
        .option(opt_email_from)
        .option(opt_email_to)
        .option(opt_notify_verbosity)
        .hook("openchannel", openchannel_hook)
        .hook("openchannel2", openchannel2_hook)
        .dynamic()
        .configure()
        .await?
    {
        Some(plugin) => {
            match read_config(plugin.configuration().lightning_dir, &plugin, &state).await {
                Ok(()) => &(),
                Err(e) => return plugin.disable(format!("{}", e).as_str()).await,
            };
            info!("read config done");
            plugin
        }
        None => return Err(anyhow!("Error configuring clnrod!")),
    };
    if let Ok(plugin) = confplugin.start(state).await {
        let aliasclone = plugin.clone();
        tokio::spawn(async move {
            time::sleep(Duration::from_secs(60 * 10)).await;
            loop {
                match tasks::refresh_alias_cache(aliasclone.clone()).await {
                    Ok(()) => (),
                    Err(e) => log::warn!("Error in refresh_alias_cache thread: {e}"),
                };
                time::sleep(Duration::from_secs(60 * 60)).await;
            }
        });
        plugin.join().await
    } else {
        Err(anyhow!("Error starting clnrod!"))
    }
}

#[derive(Parser)]
#[grammar = "rules.pest"]
pub struct RulesParser;
