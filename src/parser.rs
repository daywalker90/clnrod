#[cfg(test)]
use std::{println as warn, println as debug}; // Workaround to use prinltn! for logging in tests.

use anyhow::{anyhow, Error};
#[cfg(not(test))]
use log::{debug, warn}; // Use log crate when building application
use pest::{
    iterators::{Pair, Pairs},
    Parser,
};

use crate::{
    structs::{ClnrodParser, PeerData},
    Rule,
    RulesParser,
};

pub fn parse_rule(rule: &str) -> Result<Pairs<'_, Rule>, Error> {
    match RulesParser::parse(Rule::rule, rule) {
        Ok(mut pairs) => {
            if pairs.as_str() != rule {
                warn!(
                    "Rule invalid! Issue is somewhere here: {}",
                    rule.replace(pairs.as_str(), ""),
                );
                debug!("rule:{} pairs:{}", rule, pairs.as_str());
                return Err(anyhow!(
                    "Rule invalid! Issue is somewhere here: {}",
                    rule.replace(pairs.as_str(), ""),
                ));
            }
            Ok(pairs.next().unwrap().into_inner())
        }
        Err(e) => {
            warn!("Error parsing custom_rule: {}", e);
            Err(anyhow!("Error parsing custom_rule: {}", e))
        }
    }
}

pub fn evaluate_rule(
    parser: &ClnrodParser,
    rule: Pairs<Rule>,
    variables: &PeerData,
) -> Result<(bool, Option<String>), Error> {
    parser
        .pratt_parser
        .map_primary(|primary| match primary.as_rule() {
            Rule::comparison_expr => {
                let mut inner_pairs = primary.into_inner();
                debug!(
                    "parseCOMP: {} LEN: {}",
                    inner_pairs.as_str(),
                    inner_pairs.len()
                );
                if inner_pairs.len() == 1 {
                    // brackets detected
                    Ok(evaluate_rule(parser, inner_pairs, variables)?)
                } else {
                    let left = inner_pairs.next().unwrap();
                    let operator = inner_pairs.next().unwrap();
                    let right = inner_pairs.next().unwrap();
                    Ok(evaluate_comparison(left, right, operator, variables)?)
                }
            }
            Rule::expr => Ok(evaluate_rule(parser, primary.into_inner(), variables)?),
            other => Err(anyhow!(
                "Expected a comparison expression, got instead: `{:?}`",
                other
            )),
        })
        .map_infix(|lhs, op, rhs| match op.as_rule() {
            Rule::or => {
                let (lres, lreas) = lhs?;
                let (rres, rreas) = rhs?;
                let result = lres || rres;

                if result {
                    Ok((result, None))
                } else {
                    Ok((
                        result,
                        Some(format!("{}, {}", lreas.unwrap(), rreas.unwrap())),
                    ))
                }
            }
            Rule::and => {
                let (lres, lreas) = lhs?;
                let (rres, rreas) = rhs?;
                let result = lres && rres;
                if result {
                    Ok((result, None))
                } else if !lres && !rres {
                    Ok((
                        result,
                        Some(format!("{}, {}", lreas.unwrap(), rreas.unwrap())),
                    ))
                } else if !lres && rres {
                    Ok((result, Some(lreas.unwrap())))
                } else {
                    Ok((result, Some(rreas.unwrap())))
                }
            }
            other => Err(anyhow!(
                "Unexpected boolean operator, got instead: `{:?}`",
                other
            )),
        })
        .parse(rule)
}

fn evaluate_comparison(
    left: Pair<Rule>,
    right: Pair<Rule>,
    operator: Pair<Rule>,
    variables: &PeerData,
) -> Result<(bool, Option<String>), Error> {
    let left_value = evaluate_value(&left, variables)?;
    let right_value = evaluate_value(&right, variables)?;

    let result = match operator.as_rule() {
        Rule::equal => left_value == right_value,
        Rule::unequal => left_value != right_value,
        Rule::greater => left_value > right_value,
        Rule::lesser => left_value < right_value,
        Rule::gte => left_value >= right_value,
        Rule::lte => left_value <= right_value,
        e => return Err(anyhow!("unknown comparison operator: {:?}", e)),
    };

    let rej_match = format!("{} {} {}", left.as_str(), operator.as_str(), right_value);

    debug!("Compared: {} Result: {}", rej_match, result);

    if result {
        Ok((result, None))
    } else {
        Ok((
            result,
            Some(format!("{} -> actual: {}", rej_match, left_value)),
        ))
    }
}

fn evaluate_value(pair: &Pair<Rule>, variables: &PeerData) -> Result<u64, Error> {
    match pair.as_rule() {
        Rule::INTEGER => Ok(pair.as_str().parse::<u64>().unwrap()),
        Rule::VARIABLE => match pair.as_str() {
            p if p.eq_ignore_ascii_case("their_funding_sat") => {
                Ok(variables.openinginfo.their_funding_sat)
            }
            p if p.eq_ignore_ascii_case("cln_node_capacity_sat") => {
                Ok(variables.peerinfo.node_capacity_sat.unwrap())
            }
            p if p.eq_ignore_ascii_case("cln_channel_count") => {
                Ok(variables.peerinfo.channel_count.unwrap())
            }
            p if p.eq_ignore_ascii_case("cln_multi_channel_count") => {
                Ok(variables.openinginfo.multi_channel_count)
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
                Ok(if variables.openinginfo.channel_flags.public {
                    1
                } else {
                    0
                })
            }
            p if p.eq_ignore_ascii_case("ping") => Ok(variables.ping.unwrap() as u64),
            p if p.eq_ignore_ascii_case("oneml_capacity") => Ok(variables
                .oneml_data
                .as_ref()
                .unwrap()
                .capacity
                .unwrap_or(u64::MAX)),
            p if p.eq_ignore_ascii_case("oneml_channelcount") => Ok(variables
                .oneml_data
                .as_ref()
                .unwrap()
                .channelcount
                .unwrap_or(u64::MAX)),
            p if p.eq_ignore_ascii_case("oneml_age") => Ok(variables
                .oneml_data
                .as_ref()
                .unwrap()
                .age
                .unwrap_or(u64::MAX)),
            p if p.eq_ignore_ascii_case("oneml_growth") => Ok(variables
                .oneml_data
                .as_ref()
                .unwrap()
                .growth
                .unwrap_or(u64::MAX)),
            p if p.eq_ignore_ascii_case("oneml_availability") => Ok(variables
                .oneml_data
                .as_ref()
                .unwrap()
                .availability
                .unwrap_or(u64::MAX)),
            p if p.eq_ignore_ascii_case("amboss_capacity_rank") => {
                if let Some(metrics) = &variables
                    .amboss_data
                    .as_ref()
                    .unwrap()
                    .get_node
                    .graph_info
                    .metrics
                {
                    Ok(metrics.capacity_rank)
                } else {
                    Ok(u64::MAX)
                }
            }
            p if p.eq_ignore_ascii_case("amboss_channels_rank") => {
                if let Some(metrics) = &variables
                    .amboss_data
                    .as_ref()
                    .unwrap()
                    .get_node
                    .graph_info
                    .metrics
                {
                    Ok(metrics.channels_rank)
                } else {
                    Ok(u64::MAX)
                }
            }
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
            p if p.eq_ignore_ascii_case("amboss_terminal_web_rank") => {
                if let Some(term_web) = &variables
                    .amboss_data
                    .as_ref()
                    .unwrap()
                    .get_node
                    .socials
                    .lightning_labs
                    .terminal_web
                {
                    Ok(term_web.position)
                } else {
                    Ok(u64::MAX)
                }
            }
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
