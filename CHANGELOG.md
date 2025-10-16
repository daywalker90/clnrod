# Changelog

## [0.4.3] - 2025-10-16

### Added
- added node alias to all notifications if available

## [0.4.2] - 2025-03-11

### Changed

- upgrade dependencies

## [0.4.1] - 2024-12-10

### Changed

- upgrade dependencies

## [0.4.0] - 2024-10-20

### Added
- new custom rule variable ``ping``: check the time it takes to send a ``clnrod-pinglength`` bytes long message to the opening peer and back. Defaults to the average of 3 pings with 256 bytes length. Timeouts and errors will log but not flat out reject the channel, instead the timeout value of 5000 will be used. It is recommended to have email notifications on or watch the logs for ping timeouts (``Clnrod ping TIMEOUT``), since i encountered a rare case of CLN's ping getting stuck, requiring a node restart
- new rpc method ``clnrod-testping`` *pubkey* [*count*] [*length*]: try the ping measurements with a few options
- new option ``clnrod-pinglength``: set the length of the ping message for the custom rule check. Defaults to 256 bytes
- more precise feedback if a custom rule rejects a peer, lists offending comparisons (non-exhaustive) that caused the rejection and their actual value

## [0.3.0] - 2024-09-23

### Added

- nix flake (thanks to @RCasatta)

### Changed
- updated dependencies to fix dependabot alert for ``quinn-proto``

## [0.2.1] - 2024-06-06

### Changed

- `clnrod-denymessage` defaults to `CLNROD: Channel rejected by channel acceptor, sorry!` now, because an opener could mistake an empty message for lightning being broken.

## [0.2.0] - 2024-06-05

### Added

- Collected data appended to email body
- `clnrod-testrule`: also sending an email if configured

### Fixed

- `clnrod-testrule`: clear cache for tested pubkey first, so we fetch new data for a different custom rule
- `cln_node_capacity_sat`: was in msat precision internally
- Correctly deserialize `Amboss` API's empty strings for `amboss_has_telegram` as not having a telegram handle

### Changed

- Options code refactored. All options are now natively dynamic. Read the updated README section on how to set options for more information
- Because of the above ``cln-reload`` now only reloads the content of your ``allowlist.txt``/``denylist.txt``, everything else is handled by the new options code
- If an API returns successfully but has no data we assume the worst values instead of throwing an error

## [0.1.0] - 2024-05-02

### Added

- initial release
