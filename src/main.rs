use anyhow::anyhow;
use cln_plugin::{
    options::{ConfigOption, IntegerConfigOption, StringConfigOption},
    Builder,
};
use config::read_config;
use hooks::{openchannel2_hook, openchannel_hook};
use log::info;
use pest_derive::Parser;
use rpc::{clnrod_reload, clnrod_testmail, clnrod_testrule};
use structs::PluginState;

mod collect;
mod config;
mod hooks;
mod notify;
mod parser;
mod rpc;
mod structs;

pub const PLUGIN_NAME: &str = "clnrod";

const OPT_DENY_MESSAGE: StringConfigOption = ConfigOption::new_str_no_default(
    "clnrod-denymessage",
    "Message to send to a peer if clnrod denies a channel.",
);
const OPT_BLOCK_MODE: StringConfigOption =
    ConfigOption::new_str_no_default("clnrod-blockmode", "Use 'allow' list or 'deny' list.");
const OPT_CUSTOM_RULE: StringConfigOption =
    ConfigOption::new_str_no_default("clnrod-customrule", "Custom rule matching, see README.md");

const OPT_SMTP_USERNAME: StringConfigOption =
    ConfigOption::new_str_no_default("clnrod-smtp-username", "Set smtp username");
const OPT_SMTP_PASSWORD: StringConfigOption =
    ConfigOption::new_str_no_default("clnrod-smtp-password", "Set smtp password");
const OPT_SMTP_SERVER: StringConfigOption =
    ConfigOption::new_str_no_default("clnrod-smtp-server", "Set smtp server");
const OPT_SMTP_PORT: IntegerConfigOption =
    ConfigOption::new_i64_no_default("clnrod-smtp-port", "Set smtp port");
const OPT_EMAIL_FROM: StringConfigOption =
    ConfigOption::new_str_no_default("clnrod-email-from", "Set email_from");
const OPT_EMAIL_TO: StringConfigOption =
    ConfigOption::new_str_no_default("clnrod-email-to", "Set email_to");
const OPT_NOTIFY_VERBOSITY: StringConfigOption = ConfigOption::new_str_no_default(
    "clnrod-notify-verbosity",
    "Set the verbosity level of notifications. One of: 'ERROR', 'ACCEPTED', 'ALL'",
);

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    std::env::set_var("CLN_PLUGIN_LOG", "clnrod=debug,info");
    log_panics::init();
    let state = PluginState::new();
    let confplugin = match Builder::new(tokio::io::stdin(), tokio::io::stdout())
        .rpcmethod("clnrod-reload", "Reloads rules from file.", clnrod_reload)
        .rpcmethod("clnrod-testrule", "Test custom rule", clnrod_testrule)
        .rpcmethod("clnrod-testmail", "Test mail config", clnrod_testmail)
        .option(OPT_DENY_MESSAGE)
        .option(OPT_BLOCK_MODE)
        .option(OPT_CUSTOM_RULE)
        .option(OPT_SMTP_USERNAME)
        .option(OPT_SMTP_PASSWORD)
        .option(OPT_SMTP_SERVER)
        .option(OPT_SMTP_PORT)
        .option(OPT_EMAIL_FROM)
        .option(OPT_EMAIL_TO)
        .option(OPT_NOTIFY_VERBOSITY)
        .hook("openchannel", openchannel_hook)
        .hook("openchannel2", openchannel2_hook)
        .dynamic()
        .configure()
        .await?
    {
        Some(plugin) => {
            match read_config(plugin.configuration().lightning_dir, &state).await {
                Ok((_, _)) => &(),
                Err(e) => return plugin.disable(format!("{}", e).as_str()).await,
            };
            info!("read config done");
            // match get_startup_options(&plugin, state.clone()) {
            //     Ok(()) => &(),
            //     Err(e) => return plugin.disable(format!("{}", e).as_str()).await,
            // };
            // info!("read startup options done");
            plugin
        }
        None => return Err(anyhow!("Error configuring clnrod!")),
    };
    if let Ok(plugin) = confplugin.start(state).await {
        plugin.join().await
    } else {
        Err(anyhow!("Error starting clnrod!"))
    }
}

#[derive(Parser)]
#[grammar = "rules.pest"]
pub struct RulesParser;
