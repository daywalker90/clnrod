# Changelog

## [0.4.0] - Unreleased

### Added
- more precise feedback if custom rule denies a peer, lists offending comparisons and their actual value

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