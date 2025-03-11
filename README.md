[![latest release on CLN v25.02](https://github.com/daywalker90/clnrod/actions/workflows/latest_v25.02.yml/badge.svg?branch=main)](https://github.com/daywalker90/clnrod/actions/workflows/latest_v25.02.yml) [![latest release on CLN v24.11](https://github.com/daywalker90/clnrod/actions/workflows/latest_v24.11.yml/badge.svg?branch=main)](https://github.com/daywalker90/clnrod/actions/workflows/latest_v24.11.yml) [![latest release on CLN v24.08.2](https://github.com/daywalker90/clnrod/actions/workflows/latest_v24.08.yml/badge.svg?branch=main)](https://github.com/daywalker90/clnrod/actions/workflows/latest_v24.08.yml)

[![main on CLN v25.02](https://github.com/daywalker90/clnrod/actions/workflows/main_v25.02.yml/badge.svg?branch=main)](https://github.com/daywalker90/clnrod/actions/workflows/main_v25.02.yml) [![main on CLN v24.11](https://github.com/daywalker90/clnrod/actions/workflows/main_v24.11.yml/badge.svg?branch=main)](https://github.com/daywalker90/clnrod/actions/workflows/main_v24.11.yml) [![main on CLN v24.08.2](https://github.com/daywalker90/clnrod/actions/workflows/main_v24.08.yml/badge.svg?branch=main)](https://github.com/daywalker90/clnrod/actions/workflows/main_v24.08.yml)

# clnrod
Core lightning (CLN) plugin to allow/deny incoming channel opens based on lists and/or a custom rule.

* [Installation](#installation)
* [Building](#building)
* [Documentation](#documentation)
* [Options](#options)

# Installation
For general plugin installation instructions see the plugins repo [README.md](https://github.com/lightningd/plugins/blob/master/README.md#Installation)

Release binaries for
* x86_64-linux
* armv7-linux (Raspberry Pi 32bit)
* aarch64-linux (Raspberry Pi 64bit)

can be found on the [release](https://github.com/daywalker90/clnrod/releases) page. If you are unsure about your architecture you can run ``uname -m``.

Release binaries require ``glibc>=2.31``, which you can check with ``ldd --version``.

# Building
You can build the plugin yourself instead of using the release binaries.
First clone the repo:

```
git clone https://github.com/daywalker90/clnrod.git
```

Install a recent rust version ([rustup](https://rustup.rs/) is recommended).

Then in the ``clnrod`` folder run:

```
cargo build --release
```

After that the binary will be here: ``target/release/clnrod``

Note: Release binaries are built using ``cross`` and the ``optimized`` profile.

# Documentation
If you want to make sure that no channels open to you without going through this plugin, install it as an ``important-plugin``. CLN will stop completely if the plugin should ever crash. If you only install clnrod as a normal plugin and it crashes, all channels will be accepted as usual.
## Rpc methods
New rpc methods with this plugin:

* **clnrod-reload**
    * reload ``allowlist.txt``/``denylist.txt``
* **clnrod-testrule** *pubkey* *public* *their_funding_sat* *rule*
    * test your custom *rule* with a fake channel opening by a peer with *pubkey* who will make the channel *public* and *their_funding_sat* big
    * example: ``lightning-cli clnrod-testrule -k pubkey=02eadbd9e7557375161df8b646776a547c5cbc2e95b3071ec81553f8ec2cea3b8c public=true their_funding_sat=1000000 rule='amboss_terminal_web_rank < 1000'`` 
* **clnrod-testmail**
    * send a test mail to check your email config
* **clnrod-testping** *pubkey* [*count*] [*length*]
    * measure the time it takes in ms to send a *length* (Default: 256) bytes message to the node with *pubkey* and back. Pings *count* (Default: 3) times. You must connect to the node first!


## Blockmode: allow
Setting the blockmode to allow means:
1. if a channel opener's pubkey is on the allowlist (``~/.lightning/<network>/clnrod/allowlist.txt``) the channel will always be accepted, no matter the custom rule
2. if such a pubkey is not on the allowlist it has to face the custom rule if their is one, otherwise it will be rejected
3. if the custom rule returns ``true`` the channel will be accepted, otherwise (``false``) it will be rejected

## Blockmode: deny
Setting the blockmode to deny means:
1. if a channel opener's pubkey is on the denylist (``~/.lightning/<network>/clnrod/denylist.txt``) the channel will always be denied, no matter the custom rule
2. if such a pubkey is not on the denylist it has to face the custom rule if their is one, otherwise it will be accepted
3. if the custom rule returns ``true`` the channel will be accepted, otherwise (``false``) it will be rejected

## Logs/E-mails
Email configuration is optional and everything gets logged regardless

## Custom rule
The custom rule can make use of the following symbols:
* ``&&`` logical and
* ``||`` logical or
* ``()`` parentheses to specify the order of operations
* ``==`` equality
* ``!=`` inequality
* ``>=`` greater than or equal to
* ``<=`` smaller than or equal to
* ``>`` greater than
* ``<`` smaller than
* a boolean value is either ``true``, ``false``, ``1`` or ``0``

### Variables
Variables starting with ``cln_`` query your own gossip, ``amboss_`` the [Amboss](https://amboss.space) API and ``oneml_`` the [1ML](https://1ml.com/) API. There is an one hour cache for collecting data that will be reset if you change the ``clnrod-customrule`` option.
* ``their_funding_sat``: how much sats they are willing to open with on their side
* ``public``: if the peer intends to open the channel as public this will be ``true`` otherwise ``false``
* ``ping``: time it takes in ms to send a ``clnrod-pinglength`` (Default: 256) bytes packet to the opener and back. Timeouts and errors will log but not flat out reject the channel, instead the timeout value of 5000 will be used. It is recommended to have email notifications on or watch the logs for ping timeouts (``Clnrod ping TIMEOUT``), since i encountered a rare case of CLN's ping getting stuck, requiring a node restart
* ``cln_node_capacity_sat``: the total capacity of the peer in sats
* ``cln_channel_count``: the number of channels of the peer
* ``cln_has_clearnet``: if the peer has any clearnet addresses published this will be ``true`` otherwise ``false``
* ``cln_has_tor``: if the peer has any tor addresses published this will be ``true`` otherwise ``false``
* ``cln_anchor_support``: if the peer supports anchor channels this will be ``true`` otherwise ``false``
* ``oneml_capacity``: capacity rank from 1ML
* ``oneml_channelcount``: channel count rank from 1ML
* ``oneml_age``: age rank from 1ML
* ``oneml_growth``: growth rank from 1ML
* ``oneml_availability``: availability rank from 1ML
* ``amboss_capacity_rank``: capacity rank from amboss
* ``amboss_channels_rank``: channels rank from amboss
* ``amboss_has_email``: if this peer has published an email on amboss this will be ``true`` otherwise ``false``
* ``amboss_has_linkedin``: if this peer has published a linkedin contact on amboss this will be ``true`` otherwise ``false``
* ``amboss_has_nostr``: if this peer has published a nostr pubkey on amboss this will be ``true`` otherwise ``false``
* ``amboss_has_telegram``: if this peer has published a telegram handle on amboss this will be ``true`` otherwise ``false``
* ``amboss_has_twitter``: if this peer has published a twitter handle on amboss this will be ``true`` otherwise ``false``
* ``amboss_has_website``: if this peer has published a website address on amboss this will be ``true`` otherwise ``false``
* ``amboss_terminal_web_rank``: the [terminal.lightning](https://terminal.lightning.engineering/) rank pulled from amboss API

Example: ``their_funding_sat >= 1000000 && their_funding_sat <= 50000000 && (amboss_has_email==true || amboss_has_nostr==true)`` will accept channels that are between 1000000 and 50000000 sats in size and either have a email or nostr info on amboss

# How to set options
``clnrod`` is a dynamic plugin with dynamic options, so you can start it after CLN is already running and modify it's options after the plugin is started. You have two different methods of setting the options:

1. When starting the plugin dynamically.

* Example: ``lightning-cli -k plugin subcommand=start plugin=/path/to/clnrod clnrod-blockmode=allow``

2. Permanently saving them in the CLN config file. :warning:If you want to do this while CLN is running you must use [setconfig](https://docs.corelightning.org/reference/lightning-setconfig) instead of manually editing your config file! :warning:If you have options in the config file (either by manually editing it or by using the ``setconfig`` command) make sure the plugin will start automatically with CLN (include ``plugin=/path/to/clnrod`` or have a symlink to ``clnrod`` in your ``plugins`` folder). This is because CLN will refuse to start with config options that don't have a corresponding plugin loaded. :warning:If you edit your config file manually while CLN is running and a line changes their line number CLN will crash when you use the [setconfig](https://docs.corelightning.org/reference/lightning-setconfig) command, so better stick to ``setconfig`` only during CLN's uptime!

* Example: ``lightning-cli setconfig clnrod-blockmode allow``

You can mix two methods and if you set the same option with different methods, it will pick the value from your most recently used method.

# Options
### general
* ``clnrod-denymessage``: The custom message we will send to a rejected peer, defaults to `CLNROD: Channel rejected by channel acceptor, sorry!`
* ``clnrod-blockmode``: Set the preferred block mode to *allow* or *deny*, defaults to *deny* (with no config clnrod accepts all channels, see Documentation)
* ``clnrod-customrule``: Set the custom rule for accepting channels, see Documentation, defaults to none
* ``clnrod-pinglength``: Set the length of the ping message for the custom rule check. Defaults to `256` bytes
### email
* ``clnrod-smtp-username``: smtp username for email notifications
* ``clnrod-smtp-password``: smtp password for email notifications
* ``clnrod-smtp-server``: smtp server for email notifications
* ``clnrod-smtp-port``: smtp server port for email notifications
* ``clnrod-email-from``: email "from" field for email notifications
* ``clnrod-email-to``: email to send to for email notifications
* ``clnrod-notify-verbosity``: set verbosity level of emails to one of 
    * ``ERROR``: only errors during channel negotiation
    * ``ACCEPTED``: errors and accepted channels
    * ``ALL``: errors, accepted and rejected channels