#!/usr/bin/python

import logging

import pytest
from pyln.client import RpcError
from pyln.testing.fixtures import *  # noqa: F403
from pyln.testing.utils import sync_blockheight, wait_for
from util import experimental_anchors_check, get_plugin  # noqa: F401

LOGGER = logging.getLogger(__name__)


def test_testparse(node_factory, get_plugin):  # noqa: F811
    l1, l2 = node_factory.line_graph(
        2, wait_for_announce=True, opts=[{"plugin": get_plugin}, {}]
    )
    result = l1.rpc.call(
        "clnrod-testrule",
        {
            "rule": "public ==true && their_funding_sat>100000 && cln_channel_count>=1",
            "pubkey": l2.info["id"],
            "their_funding_sat": 200000,
            "public": True,
        },
    )
    LOGGER.info(f"{result}")
    assert result["custom_rule_result"]


def test_clnrod_custom_rule(node_factory, bitcoind, get_plugin):  # noqa: F811
    l1, l2, l3 = node_factory.get_nodes(
        3,
        opts=[
            {
                "plugin": get_plugin,
                "clnrod-customrule": "public == true && their_funding_sat>100000 && cln_channel_count>=1 && cln_multi_channel_count <= 1",
                "clnrod-denymessage": "No thanks",
            },
            {},
            {},
        ],
    )

    l2.fundwallet(10_000_000)
    chan = l2.rpc.fundchannel(l3.info["id"] + "@localhost:" + str(l3.port), 50_000)
    bitcoind.generate_block(6, wait_for_mempool=chan["txid"])
    wait_for(lambda: len(l2.rpc.listchannels()["channels"]) == 2)
    l2.rpc.connect(l1.info["id"], "localhost", l1.port)
    wait_for(lambda: len(l1.rpc.listchannels()["channels"]) == 2)

    with pytest.raises(RpcError, match="No thanks"):
        l2.rpc.fundchannel(
            l1.info["id"] + "@localhost:" + str(l1.port),
            1_000_000,
            mindepth=1,
            announce=False,
        )
    l1.daemon.wait_for_log(r"Offending comparisons: `public == 1 -> actual: 0`")

    rule2 = l1.rpc.call(
        "clnrod-testrule",
        {
            "public": True,
            "pubkey": l2.info["id"],
            "their_funding_sat": 150_000,
            "rule": "public==true && their_funding_sat>100000 && cln_channel_count>=1",
        },
    )
    assert rule2["reject_reason"] == "None"

    rule3 = l1.rpc.call(
        "clnrod-testrule",
        {
            "public": True,
            "pubkey": l2.info["id"],
            "their_funding_sat": 50_000,
            "rule": "public==true && their_funding_sat>100000 && cln_channel_count>=1",
        },
    )
    assert rule3["reject_reason"] == "their_funding_sat > 100000 -> actual: 50000"

    rule3 = l1.rpc.call(
        "clnrod-testrule",
        {
            "public": False,
            "pubkey": l2.info["id"],
            "their_funding_sat": 50_000,
            "rule": "public==true || their_funding_sat>100000 && cln_channel_count>=1",
        },
    )
    assert (
        rule3["reject_reason"]
        == "public == 1 -> actual: 0, their_funding_sat > 100000 -> actual: 50000"
    )

    rule4 = l1.rpc.call(
        "clnrod-testrule",
        {
            "public": False,
            "pubkey": l2.info["id"],
            "their_funding_sat": 50_000,
            "rule": "cln_channel_count>=1",
        },
    )
    assert rule4["reject_reason"] == "None"

    rule5 = l1.rpc.call(
        "clnrod-testrule",
        {
            "public": False,
            "pubkey": l2.info["id"],
            "their_funding_sat": 50_000,
            "rule": "cln_channel_count>1",
        },
    )
    assert rule5["reject_reason"] == "cln_channel_count > 1 -> actual: 1"

    rule6 = l1.rpc.call(
        "clnrod-testrule",
        {
            "public": True,
            "pubkey": l2.info["id"],
            "their_funding_sat": 50_000,
            "rule": "(public==true || their_funding_sat>100000) && cln_channel_count>=1",
        },
    )
    assert rule6["reject_reason"] == "None"

    rule7 = l1.rpc.call(
        "clnrod-testrule",
        {
            "public": True,
            "pubkey": l2.info["id"],
            "their_funding_sat": 50_000,
            "rule": "cln_channel_count>=1 && (public==false || their_funding_sat>100000)",
        },
    )
    assert (
        rule7["reject_reason"]
        == "public == 0 -> actual: 1, their_funding_sat > 100000 -> actual: 50000"
    )

    rule7 = l1.rpc.call(
        "clnrod-testrule",
        {
            "public": True,
            "pubkey": l2.info["id"],
            "their_funding_sat": 50_001,
            "rule": "cln_channel_count>=1 && (public==false || their_funding_sat>100000)",
        },
    )
    assert (
        rule7["reject_reason"]
        == "public == 0 -> actual: 1, their_funding_sat > 100000 -> actual: 50001"
    )

    rule8 = l1.rpc.call(
        "clnrod-testrule",
        {
            "public": True,
            "pubkey": l2.info["id"],
            "their_funding_sat": 150_002,
            "rule": "cln_channel_count>=1 && public==0",
        },
    )
    assert rule8["reject_reason"] == "public == 0 -> actual: 1"

    chan = l2.rpc.fundchannel(
        l1.info["id"] + "@localhost:" + str(l1.port),
        1_000_001,
        mindepth=1,
    )
    bitcoind.generate_block(6, wait_for_mempool=chan["txid"])
    sync_blockheight(bitcoind, [l1, l2])

    with pytest.raises(RpcError, match="No thanks"):
        l2.rpc.fundchannel(
            l1.info["id"] + "@localhost:" + str(l1.port),
            1_000_002,
            mindepth=1,
        )
    l1.daemon.wait_for_log(
        r"Offending comparisons: `cln_multi_channel_count <= 1 -> actual: 2`"
    )

    l1.rpc.setconfig("clnrod-leakreason", True)

    with pytest.raises(
        RpcError, match="No thanks Reason: cln_multi_channel_count <= 1 -> actual: 2"
    ):
        l2.rpc.fundchannel(
            l1.info["id"] + "@localhost:" + str(l1.port),
            1_000_002,
            mindepth=1,
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
    wait_for(lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) > 0)


def test_clnrod_custom_gossip(node_factory, bitcoind, get_plugin):  # noqa: F811
    opts = [
        {
            "plugin": get_plugin,
            "clnrod-customrule": "public==true && their_funding_sat>100000",
            "clnrod-denymessage": "No thanks",
        },
        {},
    ]
    if experimental_anchors_check(node_factory):
        opts[0]["experimental-anchors"] = None
        opts[1]["experimental-anchors"] = None

    l1, l2 = node_factory.get_nodes(
        2,
        opts,
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

    wait_for(lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) > 0)

    l1.rpc.setconfig(
        "clnrod-customrule",
        "public==true && their_funding_sat>100000 && cln_anchor_support==true && cln_has_tor==false && cln_channel_count>=1 && cln_node_capacity_sat>500000",
    )
    l1.rpc.call("clnrod-reload")

    wait_for(lambda: len(l1.rpc.call("listnodes", [l2.info["id"]])["nodes"]) > 0)
    wait_for(
        lambda: "features" in l1.rpc.call("listnodes", [l2.info["id"]])["nodes"][0]
    )

    l2.rpc.fundchannel(
        l1.info["id"] + "@localhost:" + str(l1.port),
        1_000_000,
        mindepth=1,
        announce=True,
    )


def test_clnrod_custom_gossip_v2(node_factory, bitcoind, get_plugin):  # noqa: F811
    opts = [
        {
            "plugin": get_plugin,
            "experimental-dual-fund": None,
            "clnrod-customrule": "public==true && their_funding_sat>100000",
            "clnrod-denymessage": "No thanks",
        },
        {
            "experimental-dual-fund": None,
        },
    ]
    if experimental_anchors_check(node_factory):
        opts[0]["experimental-anchors"] = None
        opts[1]["experimental-anchors"] = None
    l1, l2 = node_factory.get_nodes(
        2,
        opts,
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

    wait_for(lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) > 0)

    l1.rpc.setconfig(
        "clnrod-customrule",
        "public==true && their_funding_sat>100000 && cln_anchor_support==true && cln_has_tor==false && cln_channel_count>=1 && cln_node_capacity_sat>500000",
    )

    wait_for(lambda: len(l1.rpc.call("listnodes", [l2.info["id"]])["nodes"]) > 0)
    wait_for(
        lambda: "features" in l1.rpc.call("listnodes", [l2.info["id"]])["nodes"][0]
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

    l1.rpc.setconfig("clnrod-leakreason", True)

    with pytest.raises(RpcError, match="No thanks Reason: not whitelisted"):
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
    wait_for(lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) > 0)


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

    wait_for(lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) == 2)

    l1.rpc.call("clnrod-reload")

    with pytest.raises(RpcError, match="No thanks"):
        l2.rpc.fundchannel(
            l1.info["id"] + "@localhost:" + str(l1.port),
            1_000_000,
            mindepth=1,
            announce=True,
        )

    l1.rpc.setconfig("clnrod-leakreason", True)

    with pytest.raises(RpcError, match="No thanks Reason: blacklisted"):
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
        l1.rpc.listconfigs("clnrod-denymessage")["configs"]["clnrod-denymessage"][
            "value_str"
        ]
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
    with pytest.raises(RpcError, match="clnrod-smtp-port out of valid range"):
        l1.rpc.setconfig("clnrod-smtp-port", 99999)
    l1.rpc.setconfig("clnrod-smtp-port", 9999)

    with pytest.raises(RpcError, match="not a valid integer"):
        l1.rpc.setconfig("clnrod-pinglength", "test")
    with pytest.raises(RpcError, match="clnrod-pinglength out of valid range"):
        l1.rpc.setconfig("clnrod-pinglength", 99999)
    l1.rpc.setconfig("clnrod-pinglength", 9999)

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
    l1.daemon.logsearch_start = 0
    l1.daemon.wait_for_log(r"plugin-clnrod: Will try to send notifications via email")
