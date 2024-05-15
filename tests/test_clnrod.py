#!/usr/bin/python

import logging

import pytest
from pyln.client import RpcError
from pyln.testing.fixtures import *  # noqa: F403
from pyln.testing.utils import sync_blockheight, wait_for
from util import get_plugin  # noqa: F401

LOGGER = logging.getLogger(__name__)


def test_testparse(node_factory, get_plugin):  # noqa: F811
    l1, l2 = node_factory.line_graph(
        2, wait_for_announce=True, opts=[{"plugin": get_plugin}, {}]
    )
    result = l1.rpc.call(
        "clnrod-testrule",
        {
            "rule": "public==true && their_funding_sat>100000 && cln_channel_count>=1",
            "pubkey": l2.info["id"],
            "their_funding_sat": 200000,
            "public": True,
        },
    )
    LOGGER.info(f"{result}")
    assert result["custom_rule_result"]


def test_clnrod_custom_deny(node_factory, get_plugin):  # noqa: F811
    l1, l2 = node_factory.get_nodes(
        2,
        opts=[
            {
                "plugin": get_plugin,
                "clnrod-customrule": "public==true && their_funding_sat>100000 && cln_channel_count>=1",
                "clnrod-denymessage": "No thanks",
            },
            {},
        ],
    )

    l2.fundwallet(10_000_000)
    with pytest.raises(RpcError, match="No thanks"):
        l2.rpc.fundchannel(
            l1.info["id"] + "@localhost:" + str(l1.port),
            1_000_000,
            mindepth=1,
            announce=False,
        )


def test_clnrod_custom_allow(node_factory, get_plugin):  # noqa: F811
    l1, l2 = node_factory.get_nodes(
        2,
        opts=[
            {
                "plugin": get_plugin,
                "clnrod-customrule": "public==true && their_funding_sat>100000",
                "clnrod-denymessage": "No thanks",
            },
            {},
        ],
    )

    l2.fundwallet(10_000_000)
    l2.rpc.fundchannel(
        l1.info["id"] + "@localhost:" + str(l1.port),
        1_000_000,
        mindepth=1,
        announce=True,
    )
    wait_for(
        lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) > 0
    )


def test_clnrod_custom_gossip(node_factory, bitcoind, get_plugin):  # noqa: F811
    l1, l2 = node_factory.get_nodes(
        2,
        opts=[
            {
                "plugin": get_plugin,
                "experimental-anchors": None,
                "clnrod-customrule": "public==true && their_funding_sat>100000",
                "clnrod-denymessage": "No thanks",
            },
            {"experimental-anchors": None},
        ],
    )

    l2.fundwallet(10_000_000)
    l2.rpc.fundchannel(
        l1.info["id"] + "@localhost:" + str(l1.port),
        1_000_000,
        mindepth=1,
        announce=True,
    )
    bitcoind.generate_block(6)
    sync_blockheight(bitcoind, [l1, l2])

    wait_for(
        lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) > 0
    )

    l1.rpc.setconfig(
        "clnrod-customrule",
        "public==true && their_funding_sat>100000 && cln_anchor_support==true && cln_has_tor==false && cln_channel_count>=1 && cln_node_capacity_sat>500000",
    )
    l1.rpc.call("clnrod-reload")

    wait_for(
        lambda: len(l1.rpc.call("listnodes", [l2.info["id"]])["nodes"]) > 0
    )
    wait_for(
        lambda: "features"
        in l1.rpc.call("listnodes", [l2.info["id"]])["nodes"][0]
    )

    l2.rpc.fundchannel(
        l1.info["id"] + "@localhost:" + str(l1.port),
        1_000_000,
        mindepth=1,
        announce=True,
    )


def test_clnrod_custom_gossip_v2(node_factory, bitcoind, get_plugin):  # noqa: F811
    l1, l2 = node_factory.get_nodes(
        2,
        opts=[
            {
                "plugin": get_plugin,
                "experimental-dual-fund": None,
                "experimental-anchors": None,
                "clnrod-customrule": "public==true && their_funding_sat>100000",
                "clnrod-denymessage": "No thanks",
            },
            {
                "experimental-dual-fund": None,
                "experimental-anchors": None,
            },
        ],
    )

    l2.fundwallet(10_000_000)
    l2.rpc.fundchannel(
        l1.info["id"] + "@localhost:" + str(l1.port),
        1_000_000,
        mindepth=1,
        announce=True,
    )
    bitcoind.generate_block(6)
    sync_blockheight(bitcoind, [l1, l2])

    wait_for(
        lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) > 0
    )

    l1.rpc.setconfig(
        "clnrod-customrule",
        "public==true && their_funding_sat>100000 && cln_anchor_support==true && cln_has_tor==false && cln_channel_count>=1 && cln_node_capacity_sat>500000",
    )

    wait_for(
        lambda: len(l1.rpc.call("listnodes", [l2.info["id"]])["nodes"]) > 0
    )
    wait_for(
        lambda: "features"
        in l1.rpc.call("listnodes", [l2.info["id"]])["nodes"][0]
    )

    l2.rpc.fundchannel(
        l1.info["id"] + "@localhost:" + str(l1.port),
        1_000_000,
        mindepth=1,
        announce=True,
    )


def test_clnrod_allowlist(node_factory, get_plugin):  # noqa: F811
    l1, l2 = node_factory.get_nodes(
        2,
        opts=[
            {
                "plugin": get_plugin,
                "clnrod-blockmode": "allow",
                "clnrod-denymessage": "No thanks",
            },
            {},
        ],
    )

    l2.fundwallet(10_000_000)

    with pytest.raises(RpcError, match="No thanks"):
        l2.rpc.fundchannel(
            l1.info["id"] + "@localhost:" + str(l1.port),
            1_000_000,
            mindepth=1,
            announce=True,
        )

    with open(l1.info["lightning-dir"] + "/clnrod/allowlist.txt", "w") as af:
        af.writelines(l2.info["id"] + "\n")

    with pytest.raises(RpcError, match="No thanks"):
        l2.rpc.fundchannel(
            l1.info["id"] + "@localhost:" + str(l1.port),
            1_000_000,
            mindepth=1,
            announce=True,
        )

    l1.rpc.call("clnrod-reload")

    l2.rpc.fundchannel(
        l1.info["id"] + "@localhost:" + str(l1.port),
        1_000_000,
        mindepth=1,
        announce=True,
    )
    wait_for(
        lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) > 0
    )


def test_clnrod_denylist(node_factory, bitcoind, get_plugin):  # noqa: F811
    l1, l2 = node_factory.get_nodes(
        2,
        opts=[
            {
                "plugin": get_plugin,
                "clnrod-blockmode": "deny",
                "clnrod-denymessage": "No thanks",
            },
            {},
        ],
    )

    l2.fundwallet(10_000_000)

    l2.rpc.fundchannel(
        l1.info["id"] + "@localhost:" + str(l1.port),
        1_000_000,
        mindepth=1,
        announce=True,
    )
    bitcoind.generate_block(6)
    sync_blockheight(bitcoind, [l1, l2])

    with open(l1.info["lightning-dir"] + "/clnrod/denylist.txt", "w") as af:
        af.writelines(l2.info["id"] + "\n")

    l2.rpc.fundchannel(
        l1.info["id"] + "@localhost:" + str(l1.port),
        1_000_000,
        mindepth=1,
        announce=True,
    )
    bitcoind.generate_block(6)
    sync_blockheight(bitcoind, [l1, l2])

    wait_for(
        lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) == 2
    )

    l1.rpc.call("clnrod-reload")

    with pytest.raises(RpcError, match="No thanks"):
        l2.rpc.fundchannel(
            l1.info["id"] + "@localhost:" + str(l1.port),
            1_000_000,
            mindepth=1,
            announce=True,
        )


def test_options(node_factory, get_plugin):  # noqa: F811
    l1 = node_factory.get_node(options={"plugin": get_plugin})

    l1.rpc.setconfig("clnrod-denymessage", "test")
    assert (
        l1.rpc.listconfigs("clnrod-denymessage")["configs"][
            "clnrod-denymessage"
        ]["value_str"]
        == "test"
    )

    with pytest.raises(RpcError) as err:
        l1.rpc.setconfig("clnrod-blockmode", "test")
    assert err.value.error["message"] == "could not parse BlockMode from test"
    assert err.value.error["code"] == -32602
    assert (
        l1.rpc.listconfigs("clnrod-blockmode")["configs"]["clnrod-blockmode"][
            "value_str"
        ]
        != "test"
    )

    l1.rpc.setconfig("clnrod-blockmode", "allow")
    assert (
        l1.rpc.listconfigs("clnrod-blockmode")["configs"]["clnrod-blockmode"][
            "value_str"
        ]
        == "allow"
    )

    with pytest.raises(RpcError, match="Error parsing custom_rule"):
        l1.rpc.setconfig("clnrod-customrule", "test=x")
    l1.rpc.setconfig(
        "clnrod-customrule",
        "their_funding_sat >= 1000000 && their_funding_sat <= 50000000 && (amboss_has_email==true || amboss_has_nostr==true)",
    )

    with pytest.raises(RpcError, match="not a valid integer"):
        l1.rpc.setconfig("clnrod-smtp-port", "test")
    with pytest.raises(
        RpcError, match="out of range integral type conversion attempted"
    ):
        l1.rpc.setconfig("clnrod-smtp-port", 99999)
    l1.rpc.setconfig("clnrod-smtp-port", 9999)

    with pytest.raises(RpcError, match="could not parse NotifyVerbosity"):
        l1.rpc.setconfig("clnrod-notify-verbosity", "test")
    l1.rpc.setconfig("clnrod-notify-verbosity", "accepted")


def test_email_activation(node_factory, get_plugin):  # noqa: F811
    l1 = node_factory.get_node(
        options={
            "plugin": get_plugin,
            "clnrod-smtp-username": "satoshi",
            "clnrod-smtp-password": "password",
            "clnrod-smtp-server": "mail.gmx.net",
            "clnrod-smtp-port": 587,
            "clnrod-email-from": "satoshi@gmx.net",
            "clnrod-email-to": "hf@google.com",
        }
    )
    wait_for(
        lambda: l1.daemon.is_in_log(
            "plugin-clnrod: Will try to send notifications via email"
        )
    )
