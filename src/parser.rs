use anyhow::{anyhow, Error};
#[cfg(not(test))]
use log::{debug, warn}; // Use log crate when building application

use pest::{
    iterators::{Pair, Pairs},
    Parser,
};
#[cfg(test)]
use std::{println as warn, println as debug}; // Workaround to use prinltn! for logging in tests.

use crate::{structs::PeerData, Rule, RulesParser};

pub fn parse_rule(rule: &str) -> Result<Pairs<Rule>, Error> {
    match RulesParser::parse(Rule::expression, rule) {
        Ok(pairs) => {
            if pairs.as_str() != rule {
                warn!(
                    "Rule invalid! Issue is somewhere here: {}",
                    rule.replace(pairs.as_str(), ""),
                );
                return Err(anyhow!(
                    "Rule invalid! Issue is somewhere here: {}",
                    rule.replace(pairs.as_str(), ""),
                ));
            }
            Ok(pairs)
        }
        Err(e) => {
            warn!("Error parsing custom_rule: {}", e);
            Err(anyhow!("Error parsing custom_rule: {}", e))
        }
    }
}

pub fn evaluate_rule(rule: Pairs<Rule>, variables: &PeerData) -> Result<bool, Error> {
    for pair in rule {
        debug!("parseCond: {}", pair.as_str());
        if !evaluate_expression(pair, variables)? {
            debug!("parseCond: false");
            return Ok(false);
        }
    }
    debug!("parseCond: true");
    Ok(true)
}

fn evaluate_expression(pair: Pair<Rule>, _variables: &PeerData) -> Result<bool, Error> {
    match pair.as_rule() {
        Rule::and_expr => {
            let mut result = true;
            for inner_pair in pair.into_inner() {
                debug!("parseAND: {}", inner_pair.as_str());
                result = result && evaluate_expression(inner_pair, _variables)?;
                debug!("parseAND: {}", result);
            }
            Ok(result)
        }
        Rule::or_expr => {
            let mut result = false;
            for inner_pair in pair.into_inner() {
                debug!("parseOR: {}", inner_pair.as_str());
                result = result || evaluate_expression(inner_pair, _variables)?;
                debug!("parseOR: {}", result);
            }
            Ok(result)
        }
        Rule::comparison_expr => {
            let mut inner_pairs = pair.into_inner();
            debug!(
                "parseCOMP: {} LEN: {}",
                inner_pairs.as_str(),
                inner_pairs.len()
            );
            if inner_pairs.len() == 1 {
                // brackets detected
                Ok(evaluate_rule(inner_pairs, _variables)?)
            } else {
                let left = inner_pairs.next().unwrap();
                let operator = inner_pairs.next().unwrap().as_str();
                let right = inner_pairs.next().unwrap();
                Ok(evaluate_comparison(left, right, operator, _variables)?)
            }
        }
        e => {
            warn!("parseERR: {}, {:?}", pair, e);
            Err(anyhow!("parseERR: {}, {:?}", pair, e))
        }
    }
}

fn evaluate_comparison(
    left: Pair<Rule>,
    right: Pair<Rule>,
    operator: &str,
    variables: &PeerData,
) -> Result<bool, Error> {
    let left_value = evaluate_value(&left, variables)?;
    let right_value = evaluate_value(&right, variables)?;
    debug!("COMP: {} {} {}", left_value, operator, right_value);
    match operator {
        "==" => Ok(left_value == right_value),
        "!=" => Ok(left_value != right_value),
        ">" => Ok(left_value > right_value),
        "<" => Ok(left_value < right_value),
        ">=" => Ok(left_value >= right_value),
        "<=" => Ok(left_value <= right_value),
        e => Err(anyhow!("unknown comparison operator: {}", e)),
    }
}

fn evaluate_value(pair: &Pair<Rule>, variables: &PeerData) -> Result<u64, Error> {
    match pair.as_rule() {
        Rule::INTEGER => Ok(pair.as_str().parse::<u64>().unwrap()),
        Rule::VARIABLE => match pair.as_str() {
            p if p.eq_ignore_ascii_case("their_funding_sat") => {
                Ok(variables.peerinfo.their_funding_sat)
            }
            p if p.eq_ignore_ascii_case("cln_node_capacity_sat") => {
                Ok(variables.peerinfo.node_capacity_sat.unwrap())
            }
            p if p.eq_ignore_ascii_case("cln_channel_count") => {
                Ok(variables.peerinfo.channel_count.unwrap())
            }
            p if p.eq_ignore_ascii_case("cln_has_clearnet") => {
                Ok(if variables.peerinfo.has_clearnet.unwrap() {
                    1
                } else {
                    0
                })
            }
            p if p.eq_ignore_ascii_case("cln_has_tor") => {
                Ok(if variables.peerinfo.has_tor.unwrap() {
                    1
                } else {
                    0
                })
            }
            p if p.eq_ignore_ascii_case("cln_anchor_support") => {
                Ok(if variables.peerinfo.anchor_support.unwrap() {
                    1
                } else {
                    0
                })
            }
            p if p.eq_ignore_ascii_case("public") => {
                Ok(if variables.peerinfo.channel_flags.public {
                    1
                } else {
                    0
                })
            }
            p if p.eq_ignore_ascii_case("oneml_capacity") => {
                Ok(variables.oneml_data.as_ref().unwrap().capacity)
            }
            p if p.eq_ignore_ascii_case("oneml_channelcount") => {
                Ok(variables.oneml_data.as_ref().unwrap().channelcount)
            }
            p if p.eq_ignore_ascii_case("oneml_age") => {
                Ok(variables.oneml_data.as_ref().unwrap().age)
            }
            p if p.eq_ignore_ascii_case("oneml_growth") => {
                Ok(variables.oneml_data.as_ref().unwrap().growth)
            }
            p if p.eq_ignore_ascii_case("oneml_availability") => {
                Ok(variables.oneml_data.as_ref().unwrap().availability)
            }
            p if p.eq_ignore_ascii_case("amboss_capacity_rank") => Ok(variables
                .amboss_data
                .as_ref()
                .unwrap()
                .get_node
                .graph_info
                .metrics
                .as_ref()
                .ok_or_else(|| anyhow!("amboss_capacity_rank: no metrics found"))?
                .capacity_rank),
            p if p.eq_ignore_ascii_case("amboss_channels_rank") => Ok(variables
                .amboss_data
                .as_ref()
                .unwrap()
                .get_node
                .graph_info
                .metrics
                .as_ref()
                .ok_or_else(|| anyhow!("amboss_channels_rank: no metrics found"))?
                .channels_rank),
            p if p.eq_ignore_ascii_case("amboss_has_email") => Ok(
                if variables
                    .amboss_data
                    .as_ref()
                    .unwrap()
                    .get_node
                    .socials
                    .info
                    .as_ref()
                    .is_some_and(|i| i.email.is_some())
                {
                    1
                } else {
                    0
                },
            ),
            p if p.eq_ignore_ascii_case("amboss_has_linkedin") => Ok(
                if variables
                    .amboss_data
                    .as_ref()
                    .unwrap()
                    .get_node
                    .socials
                    .info
                    .as_ref()
                    .is_some_and(|i| i.linkedin.is_some())
                {
                    1
                } else {
                    0
                },
            ),
            p if p.eq_ignore_ascii_case("amboss_has_nostr") => Ok(
                if variables
                    .amboss_data
                    .as_ref()
                    .unwrap()
                    .get_node
                    .socials
                    .info
                    .as_ref()
                    .is_some_and(|i| i.nostr.is_some())
                {
                    1
                } else {
                    0
                },
            ),
            p if p.eq_ignore_ascii_case("amboss_has_telegram") => Ok(
                if variables
                    .amboss_data
                    .as_ref()
                    .unwrap()
                    .get_node
                    .socials
                    .info
                    .as_ref()
                    .is_some_and(|i| i.telegram.is_some())
                {
                    1
                } else {
                    0
                },
            ),
            p if p.eq_ignore_ascii_case("amboss_has_twitter") => Ok(
                if variables
                    .amboss_data
                    .as_ref()
                    .unwrap()
                    .get_node
                    .socials
                    .info
                    .as_ref()
                    .is_some_and(|i| i.twitter.is_some())
                {
                    1
                } else {
                    0
                },
            ),
            p if p.eq_ignore_ascii_case("amboss_has_website") => Ok(
                if variables
                    .amboss_data
                    .as_ref()
                    .unwrap()
                    .get_node
                    .socials
                    .info
                    .as_ref()
                    .is_some_and(|i| i.website.is_some())
                {
                    1
                } else {
                    0
                },
            ),
            p if p.eq_ignore_ascii_case("amboss_terminal_web_rank") => Ok(variables
                .amboss_data
                .as_ref()
                .unwrap()
                .get_node
                .socials
                .lightning_labs
                .terminal_web
                .as_ref()
                .ok_or_else(|| anyhow!("amboss_terminal_web_rank: no metrics found"))?
                .position),
            _ => Err(anyhow!("Invalid variable name: {}", pair.as_str())),
        },
        Rule::BOOLEAN => match pair.as_str() {
            v if v.eq_ignore_ascii_case("true") => Ok(1),
            v if v.eq_ignore_ascii_case("false") => Ok(0),
            e => Err(anyhow!("Invalid Boolean atomic: {}", e)),
        },
        Rule::value => {
            let mut inner_pairs = pair.clone().into_inner();
            let inner = inner_pairs.next().unwrap();
            evaluate_value(&inner, variables)
        }
        e => Err(anyhow!("Unexpected rule:{:?}", e)),
    }
}
