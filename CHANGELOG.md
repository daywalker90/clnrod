# Changelog

## [Unreleased]

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

## [0.1.0] - 2024-05-02

### Added

- initial release