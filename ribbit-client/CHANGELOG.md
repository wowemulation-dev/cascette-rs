# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Refactored BPSV response handling:
  - Split BPSV functionality from `TypedResponse` to new `TypedBpsvResponse` trait
  - Allows non-BPSV responses to be parsed through the typed response system
  - Re-exported `TypedBpsvResponse` for backward compatibility
  - Improved separation of concerns between response types

## [0.1.2](https://github.com/wowemulation-dev/cascette-rs/compare/ribbit-client-v0.1.1...ribbit-client-v0.1.2) - 2025-07-15

### Other

- updated the following local packages: ngdp-bpsv

## [0.1.1](https://github.com/wowemulation-dev/cascette-rs/compare/ribbit-client-v0.1.0...ribbit-client-v0.1.1) - 2025-06-29

### Other

- updated the following local packages: ngdp-bpsv
