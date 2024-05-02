#!/usr/bin/python

import logging

from pyln.client import RpcError
from pyln.testing.fixtures import *  # noqa: F403
from pyln.testing.utils import sync_blockheight, wait_for
from util import get_plugin, update_config_file_option  # noqa: F401

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
    assert result["parse_result"]


def test_clnrod_custom_deny(node_factory, get_plugin):  # noqa: F811
    l1, l2 = node_factory.get_nodes(
        2,
        opts=[
            {
                "plugin": get_plugin,
            },
            {},
        ],
    )
    with open(l1.info["lightning-dir"] + "/config", "a") as af:
        af.writelines([
            "clnrod-customrule=public==true && their_funding_sat>100000 && cln_channel_count>=1\n",
            "clnrod-denymessage=No thanks\n",
        ])
    l1.rpc.call("clnrod-reload")

    l2.fundwallet(10_000_000)
    l2.rpc.connect(l1.info["id"], "localhost", l1.port)
    try:
        l2.rpc.fundchannel(l1.info["id"], 1_000_000, mindepth=1, announce=False)
    except RpcError as err:
        assert "No thanks" in err.error["message"]


def test_clnrod_custom_allow(node_factory, get_plugin):  # noqa: F811
    l1, l2 = node_factory.get_nodes(
        2,
        opts=[
            {
                "plugin": get_plugin,
            },
            {},
        ],
    )
    with open(l1.info["lightning-dir"] + "/config", "a") as af:
        af.writelines([
            "clnrod-customrule=public==true && their_funding_sat>100000\n",
            "clnrod-denymessage=No thanks\n",
        ])
    l1.rpc.call("clnrod-reload")

    l2.fundwallet(10_000_000)
    l2.rpc.connect(l1.info["id"], "localhost", l1.port)
    l2.rpc.fundchannel(l1.info["id"], 1_000_000, mindepth=1, announce=True)
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
            },
            {"experimental-anchors": None},
        ],
    )
    with open(l1.info["lightning-dir"] + "/config", "a") as af:
        af.writelines([
            "clnrod-customrule=public==true && their_funding_sat>100000\n",
            "clnrod-denymessage=No thanks\n",
        ])
    l1.rpc.call("clnrod-reload")

    l2.fundwallet(10_000_000)
    l2.rpc.connect(l1.info["id"], "localhost", l1.port)
    l2.rpc.fundchannel(l1.info["id"], 1_000_000, mindepth=1, announce=True)
    bitcoind.generate_block(6)
    sync_blockheight(bitcoind, [l1, l2])

    wait_for(
        lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) > 0
    )

    update_config_file_option(
        l1.info["lightning-dir"],
        "clnrod-customrule",
        "public==true && their_funding_sat>100000 && cln_anchor_support==true && cln_has_tor==false && cln_channel_count>=1 && cln_node_capacity_sat>500000",
    )
    l1.rpc.call("clnrod-reload")

    wait_for(
        lambda: "features"
        in l1.rpc.call("listnodes", [l2.info["id"]])["nodes"][0]
    )

    l2.rpc.fundchannel(l1.info["id"], 1_000_000, mindepth=1, announce=True)


def test_clnrod_custom_gossip_v2(node_factory, bitcoind, get_plugin):  # noqa: F811
    l1, l2 = node_factory.get_nodes(
        2,
        opts=[
            {
                "plugin": get_plugin,
                "experimental-dual-fund": None,
                "experimental-anchors": None,
            },
            {
                "experimental-dual-fund": None,
                "experimental-anchors": None,
            },
        ],
    )
    with open(l1.info["lightning-dir"] + "/config", "a") as af:
        af.writelines([
            "clnrod-customrule=public==true && their_funding_sat>100000\n",
            "clnrod-denymessage=No thanks\n",
        ])
    l1.rpc.call("clnrod-reload")

    l2.fundwallet(10_000_000)
    l2.rpc.connect(l1.info["id"], "localhost", l1.port)
    l2.rpc.fundchannel(l1.info["id"], 1_000_000, mindepth=1, announce=True)
    bitcoind.generate_block(6)
    sync_blockheight(bitcoind, [l1, l2])

    wait_for(
        lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) > 0
    )

    update_config_file_option(
        l1.info["lightning-dir"],
        "clnrod-customrule",
        "public==true && their_funding_sat>100000 && cln_anchor_support==true && cln_has_tor==false && cln_channel_count>=1 && cln_node_capacity_sat>500000",
    )
    l1.rpc.call("clnrod-reload")

    wait_for(
        lambda: "features"
        in l1.rpc.call("listnodes", [l2.info["id"]])["nodes"][0]
    )

    l2.rpc.fundchannel(l1.info["id"], 1_000_000, mindepth=1, announce=True)


def test_clnrod_allowlist(node_factory, get_plugin):  # noqa: F811
    l1, l2 = node_factory.get_nodes(
        2,
        opts=[
            {
                "plugin": get_plugin,
            },
            {},
        ],
    )
    with open(l1.info["lightning-dir"] + "/config", "a") as af:
        af.writelines([
            "clnrod-blockmode=allow\n",
            "clnrod-denymessage=No thanks\n",
        ])
    l1.rpc.call("clnrod-reload")

    l2.fundwallet(10_000_000)
    l2.rpc.connect(l1.info["id"], "localhost", l1.port)

    try:
        l2.rpc.fundchannel(l1.info["id"], 1_000_000, mindepth=1, announce=True)
    except RpcError as err:
        assert "No thanks" in err.error["message"]

    with open(l1.info["lightning-dir"] + "/clnrod/allowlist.txt", "w") as af:
        af.writelines(l2.info["id"] + "\n")

    try:
        l2.rpc.fundchannel(l1.info["id"], 1_000_000, mindepth=1, announce=True)
    except RpcError as err:
        assert "No thanks" in err.error["message"]

    l1.rpc.call("clnrod-reload")

    l2.rpc.fundchannel(l1.info["id"], 1_000_000, mindepth=1, announce=True)
    wait_for(
        lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) > 0
    )


def test_clnrod_denylist(node_factory, bitcoind, get_plugin):  # noqa: F811
    l1, l2 = node_factory.get_nodes(
        2,
        opts=[
            {
                "plugin": get_plugin,
            },
            {},
        ],
    )
    with open(l1.info["lightning-dir"] + "/config", "a") as af:
        af.writelines([
            "clnrod-blockmode=deny\n",
            "clnrod-denymessage=No thanks\n",
        ])
    l1.rpc.call("clnrod-reload")

    l2.fundwallet(10_000_000)
    l2.rpc.connect(l1.info["id"], "localhost", l1.port)

    l2.rpc.fundchannel(l1.info["id"], 1_000_000, mindepth=1, announce=True)
    bitcoind.generate_block(6)
    sync_blockheight(bitcoind, [l1, l2])

    with open(l1.info["lightning-dir"] + "/clnrod/denylist.txt", "w") as af:
        af.writelines(l2.info["id"] + "\n")

    l2.rpc.fundchannel(l1.info["id"], 1_000_000, mindepth=1, announce=True)
    bitcoind.generate_block(6)
    sync_blockheight(bitcoind, [l1, l2])

    wait_for(
        lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) == 2
    )

    l1.rpc.call("clnrod-reload")

    try:
        l2.rpc.fundchannel(l1.info["id"], 1_000_000, mindepth=1, announce=True)
    except RpcError as err:
        assert "No thanks" in err.error["message"]
