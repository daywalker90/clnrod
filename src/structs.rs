use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::Arc,
};

use crate::{
    OPT_BLOCK_MODE, OPT_CUSTOM_RULE, OPT_DENY_MESSAGE, OPT_EMAIL_FROM, OPT_EMAIL_TO,
    OPT_NOTIFY_VERBOSITY, OPT_SMTP_PASSWORD, OPT_SMTP_PORT, OPT_SMTP_SERVER, OPT_SMTP_USERNAME,
};
use anyhow::{anyhow, Error};
use cln_rpc::primitives::PublicKey;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct PluginState {
    pub config: Arc<Mutex<Config>>,
    pub pubkey_list: Arc<Mutex<HashSet<PublicKey>>>,
    pub amboss_lock: Arc<tokio::sync::Mutex<u128>>,
    pub oneml_lock: Arc<tokio::sync::Mutex<u128>>,
    pub peerdata_cache: Arc<Mutex<HashMap<PublicKey, PeerDataCache>>>,
}
impl PluginState {
    pub fn new() -> PluginState {
        PluginState {
            config: Arc::new(Mutex::new(Config::new())),
            pubkey_list: Arc::new(Mutex::new(HashSet::new())),
            amboss_lock: Arc::new(tokio::sync::Mutex::new(0)),
            oneml_lock: Arc::new(tokio::sync::Mutex::new(0)),
            peerdata_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[derive(Clone, Debug)]
pub enum BlockMode {
    Allow,
    Deny,
}
impl FromStr for BlockMode {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "allow" => Ok(BlockMode::Allow),
            "deny" => Ok(BlockMode::Deny),
            _ => Err(anyhow!("could not parse BlockMode from {}", s)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub deny_message: DynamicConfigOption<String>,
    pub block_mode: DynamicConfigOption<BlockMode>,
    pub custom_rule: DynamicConfigOption<String>,
    pub smtp_username: DynamicConfigOption<String>,
    pub smtp_password: DynamicConfigOption<String>,
    pub smtp_server: DynamicConfigOption<String>,
    pub smtp_port: DynamicConfigOption<u16>,
    pub email_from: DynamicConfigOption<String>,
    pub email_to: DynamicConfigOption<String>,
    pub send_mail: bool,
    pub notify_verbosity: DynamicConfigOption<NotifyVerbosity>,
}
impl Config {
    pub fn new() -> Config {
        Config {
            deny_message: DynamicConfigOption {
                name: OPT_DENY_MESSAGE,
                value: String::new(),
            },
            block_mode: DynamicConfigOption {
                name: OPT_BLOCK_MODE,
                value: BlockMode::Deny,
            },
            custom_rule: DynamicConfigOption {
                name: OPT_CUSTOM_RULE,
                value: String::new(),
            },
            smtp_username: DynamicConfigOption {
                name: OPT_SMTP_USERNAME,
                value: String::new(),
            },
            smtp_password: DynamicConfigOption {
                name: OPT_SMTP_PASSWORD,
                value: String::new(),
            },
            smtp_server: DynamicConfigOption {
                name: OPT_SMTP_SERVER,
                value: String::new(),
            },
            smtp_port: DynamicConfigOption {
                name: OPT_SMTP_PORT,
                value: 0,
            },
            email_from: DynamicConfigOption {
                name: OPT_EMAIL_FROM,
                value: String::new(),
            },
            email_to: DynamicConfigOption {
                name: OPT_EMAIL_TO,
                value: String::new(),
            },
            send_mail: false,
            notify_verbosity: DynamicConfigOption {
                name: OPT_NOTIFY_VERBOSITY,
                value: NotifyVerbosity::All,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct DynamicConfigOption<T> {
    pub name: &'static str,
    pub value: T,
}

#[derive(Clone, Debug)]
pub struct PeerDataCache {
    pub peer_data: PeerData,
    pub age: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerData {
    pub peerinfo: PeerInfo,
    pub oneml_data: Option<OneMl>,
    pub amboss_data: Option<AmbossNodeData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfo {
    pub pubkey: PublicKey,
    pub their_funding_sat: u64,
    pub channel_flags: ChannelFlags,
    pub channel_count: Option<u64>,
    pub node_capacity_sat: Option<u64>,
    pub has_clearnet: Option<bool>,
    pub has_tor: Option<bool>,
    pub anchor_support: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelFlags {
    pub public: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OneMl {
    pub capacity: u64,
    pub channelcount: u64,
    pub age: u64,
    pub growth: u64,
    pub availability: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbossResponse {
    pub data: AmbossNodeData,
    pub extensions: AmbossExtensions,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbossNodeData {
    #[serde(rename = "getNode")]
    pub get_node: AmbossData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbossData {
    pub graph_info: AmbossGraphInfo,
    pub socials: AmbossSocials,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbossGraphInfo {
    pub metrics: Option<AmbossGraphInfoMetrics>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbossGraphInfoMetrics {
    pub capacity_rank: u64,
    pub channels_rank: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbossSocials {
    pub info: Option<AmbossSocialsInfo>,
    pub lightning_labs: AmbossLightningLabs,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbossSocialsInfo {
    pub email: Option<String>,
    pub linkedin: Option<String>,
    pub nostr: Option<String>,
    pub telegram: Option<String>,
    pub twitter: Option<String>,
    pub website: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbossLightningLabs {
    pub terminal_web: Option<AmbossLightningLabsTerminalWeb>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbossLightningLabsTerminalWeb {
    pub position: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbossExtensions {
    pub cost: AmbossExtensionsCost,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbossExtensionsCost {
    #[serde(rename = "requestedQueryCost")]
    pub requested_query_cost: u64,
    #[serde(rename = "throttleStatus")]
    pub throttle_status: AmbossExtensionsCostThrottleStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbossExtensionsCostThrottleStatus {
    #[serde(rename = "maximumAvailable")]
    pub maximum_available: u64,
    #[serde(rename = "currentlyAvailable")]
    pub currently_available: u64,
    #[serde(rename = "restoreRate")]
    pub restore_rate: u64,
}

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum NotifyVerbosity {
    Error,
    Accepted,
    All,
}
impl FromStr for NotifyVerbosity {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "error" => Ok(NotifyVerbosity::Error),
            "accepted" => Ok(NotifyVerbosity::Accepted),
            "all" => Ok(NotifyVerbosity::All),
            _ => Err(anyhow!("could not parse NotifyVerbosity from {}", s)),
        }
    }
}
