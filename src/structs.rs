use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Display, Formatter, Write as _},
    str::FromStr,
    sync::Arc,
};

use anyhow::{anyhow, Error};
use cln_rpc::primitives::PublicKey;
use parking_lot::Mutex;
use pest::pratt_parser::{Assoc, Op, PrattParser};
use serde::{de::IntoDeserializer, Deserialize, Serialize};

use crate::Rule;

#[derive(Clone)]
pub struct PluginState {
    pub config: Arc<Mutex<Config>>,
    pub pubkey_list: Arc<Mutex<HashSet<PublicKey>>>,
    pub amboss_lock: Arc<tokio::sync::Mutex<u128>>,
    pub oneml_lock: Arc<tokio::sync::Mutex<u128>>,
    pub peerdata_cache: Arc<Mutex<HashMap<PublicKey, PeerDataCache>>>,
    pub alias_cache: Arc<Mutex<HashMap<PublicKey, String>>>,
}
impl PluginState {
    pub fn new() -> PluginState {
        PluginState {
            config: Arc::new(Mutex::new(Config::new())),
            pubkey_list: Arc::new(Mutex::new(HashSet::new())),
            amboss_lock: Arc::new(tokio::sync::Mutex::new(0)),
            oneml_lock: Arc::new(tokio::sync::Mutex::new(0)),
            peerdata_cache: Arc::new(Mutex::new(HashMap::new())),
            alias_cache: Arc::new(Mutex::new(HashMap::new())),
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
            _ => Err(anyhow!("could not parse BlockMode from {s}")),
        }
    }
}
impl Display for BlockMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            BlockMode::Allow => write!(f, "allow"),
            BlockMode::Deny => write!(f, "deny"),
        }
    }
}

pub struct ClnrodParser {
    pub pratt_parser: PrattParser<Rule>,
}
impl ClnrodParser {
    pub fn new() -> ClnrodParser {
        ClnrodParser {
            pratt_parser: PrattParser::new()
                .op(Op::infix(Rule::or, Assoc::Left))
                .op(Op::infix(Rule::and, Assoc::Left)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub deny_message: String,
    pub leak_reason: bool,
    pub block_mode: BlockMode,
    pub custom_rule: String,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_server: String,
    pub smtp_port: u16,
    pub email_from: String,
    pub email_to: String,
    pub send_mail: bool,
    pub notify_verbosity: NotifyVerbosity,
    pub ping_length: u16,
}
impl Config {
    pub fn new() -> Config {
        Config {
            deny_message: "CLNROD: Channel rejected by channel acceptor, sorry!".to_string(),
            leak_reason: false,
            block_mode: BlockMode::Deny,
            custom_rule: String::new(),
            smtp_username: String::new(),
            smtp_password: String::new(),
            smtp_server: String::new(),
            smtp_port: 0,
            email_from: String::new(),
            email_to: String::new(),
            send_mail: false,
            notify_verbosity: NotifyVerbosity::All,
            ping_length: 256,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PeerDataCache {
    pub peer_data: PeerData,
    pub age: u64,
}

impl Display for PeerDataCache {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut result = format!(
            "their_funding_sat: {}\npublic: {}",
            self.peer_data.openinginfo.their_funding_sat,
            self.peer_data.openinginfo.channel_flags.public
        );
        if let Some(p) = self.peer_data.ping {
            write!(result, "\nping: {p}")?;
        }
        if let Some(c) = self.peer_data.peerinfo.node_capacity_sat {
            write!(result, "\ncln_node_capacity_sat: {c}")?;
        }
        if let Some(c) = self.peer_data.peerinfo.channel_count {
            write!(result, "\ncln_channel_count: {c}")?;
        }
        write!(
            result,
            "\ncln_multi_channel_count: {}",
            self.peer_data.openinginfo.multi_channel_count
        )?;
        if let Some(c) = self.peer_data.peerinfo.has_clearnet {
            write!(result, "\ncln_has_clearnet: {c}")?;
        }
        if let Some(c) = self.peer_data.peerinfo.has_tor {
            write!(result, "\ncln_has_tor: {c}")?;
        }
        if let Some(c) = self.peer_data.peerinfo.anchor_support {
            write!(result, "\ncln_anchor_support: {c}")?;
        }

        if let Some(oneml_data) = &self.peer_data.oneml_data {
            write!(
                result,
                "\noneml_capacity: {}",
                oneml_data.capacity.unwrap_or(u64::MAX)
            )?;
            write!(
                result,
                "\noneml_channelcount: {}",
                oneml_data.channelcount.unwrap_or(u64::MAX)
            )?;
            write!(
                result,
                "\noneml_age: {}",
                oneml_data.age.unwrap_or(u64::MAX)
            )?;
            write!(
                result,
                "\noneml_growth: {}",
                oneml_data.growth.unwrap_or(u64::MAX)
            )?;
            write!(
                result,
                "\noneml_availability: {}",
                oneml_data.availability.unwrap_or(u64::MAX)
            )?;
        }

        if let Some(amboss_data) = &self.peer_data.amboss_data {
            if let Some(amboss_metrics) = &amboss_data.get_node.graph_info.metrics {
                write!(
                    result,
                    "\namboss_capacity_rank: {}",
                    amboss_metrics.capacity_rank
                )?;
                write!(
                    result,
                    "\namboss_channels_rank: {}",
                    amboss_metrics.channels_rank
                )?;
            } else {
                result.push_str("\namboss_capacity_rank: None\namboss_channels_rank: None");
            }
            if let Some(amboss_socials) = &amboss_data.get_node.socials.info {
                if let Some(c) = &amboss_socials.email {
                    write!(result, "\namboss_has_email: {c}")?;
                }
                if let Some(c) = &amboss_socials.linkedin {
                    write!(result, "\namboss_has_linkedin: {c}")?;
                }
                if let Some(c) = &amboss_socials.nostr {
                    write!(result, "\namboss_has_nostr: {c}")?;
                }
                if let Some(c) = &amboss_socials.telegram {
                    write!(result, "\namboss_has_telegram: {c}")?;
                }
                if let Some(c) = &amboss_socials.twitter {
                    write!(result, "\namboss_has_twitter: {c}")?;
                }
                if let Some(c) = &amboss_socials.website {
                    write!(result, "\namboss_has_website: {c}")?;
                }
            } else {
                result.push_str("\nNo socials found on amboss.");
            }
            if let Some(amboss_ll) = &amboss_data.get_node.socials.lightning_labs.terminal_web {
                write!(result, "\namboss_terminal_web_rank: {}", amboss_ll.position)?;
            } else {
                result.push_str("\nNo Terminal Web data found on amboss.");
            }
            if !result.contains("amboss_") {
                result.push_str("\nAmboss does not have any data for this node!");
            }
        }
        write!(f, "{result}")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerData {
    pub ping: Option<u16>,
    pub peerinfo: PeerInfo,
    pub openinginfo: OpeningInfo,
    pub oneml_data: Option<OneMl>,
    pub amboss_data: Option<AmbossNodeData>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PeerInfo {
    pub pubkey: PublicKey,
    pub channel_count: Option<u64>,
    pub node_capacity_sat: Option<u64>,
    pub has_clearnet: Option<bool>,
    pub has_tor: Option<bool>,
    pub anchor_support: Option<bool>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct OpeningInfo {
    pub their_funding_sat: u64,
    pub multi_channel_count: u64,
    pub channel_flags: ChannelFlags,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ChannelFlags {
    pub public: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct OneMl {
    pub capacity: Option<u64>,
    pub channelcount: Option<u64>,
    pub age: Option<u64>,
    pub growth: Option<u64>,
    pub availability: Option<u64>,
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AmbossGraphInfo {
    pub metrics: Option<AmbossGraphInfoMetrics>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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
    #[serde(deserialize_with = "empty_string_as_none")]
    pub email: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none")]
    pub linkedin: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none")]
    pub nostr: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none")]
    pub telegram: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none")]
    pub twitter: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none")]
    pub website: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AmbossLightningLabs {
    pub terminal_web: Option<AmbossLightningLabsTerminalWeb>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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
            _ => Err(anyhow!("could not parse NotifyVerbosity from {s}")),
        }
    }
}
impl Display for NotifyVerbosity {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            NotifyVerbosity::Error => write!(f, "error"),
            NotifyVerbosity::Accepted => write!(f, "accepted"),
            NotifyVerbosity::All => write!(f, "all"),
        }
    }
}

fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    let opt = Option::<String>::deserialize(de)?;
    let opt = opt.as_deref();
    match opt {
        None | Some("") => Ok(None),
        Some(s) => T::deserialize(s.into_deserializer()).map(Some),
    }
}
