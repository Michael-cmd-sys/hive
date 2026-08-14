# Changelog

All notable changes to this project are documented here, grouped by release.
This project follows [Conventional Commits](https://www.conventionalcommits.org).
## [unreleased]

### Bug Fixes

- *(ci)* Correct git-cliff action repo to orhun/git-cliff-action
- *(ci)* Install zig via mlugg/setup-zig instead of apt
- *(ci)* Changelog workflow must stage new CHANGELOG.md before diffing
- *(ui)* Give bottom bar 2 rows so input/help text renders
- *(ui)* Vertically center the HIVE banner on the splash screen
- Add missing Dioxus site src + explicit bin target (retro CRT theme)

### CI

- Add git-cliff + cargo-release versioning and cross-platform release workflow
- Force bash shell for artifact staging step (windows runner defaults to pwsh)

### Documentation

- Link live landing page in README + fix stray char

### Features

- Add hive ASCII banner on Machines (landing) tab
- Delete/wipe machines, input validation, in-memory passwords, HIVE banner
- Vim + arrow-key navigation (h/l tabs, j/k list, ←/→ tabs)
- Launch splash screen with the HIVE banner
- Visible run/MPI scope toggle (t) — all nodes or selected machine
- Monitor gauges + per-node process breakdown
- Add Dioxus landing site + GitHub Pages deploy workflow

### Miscellaneous

- Update CHANGELOG [skip ci]
- Update CHANGELOG [skip ci]
- Update CHANGELOG [skip ci]
- Update CHANGELOG [skip ci]
- Update CHANGELOG [skip ci]
- Update CHANGELOG [skip ci]
- Update CHANGELOG [skip ci]
- Update CHANGELOG [skip ci]
- Update CHANGELOG [skip ci]
- Update CHANGELOG [skip ci]
- Update CHANGELOG [skip ci]

### Ux

- Add interactive add-machine wizard, command/MPI input, help bar; drop ascii banner; fix windows build via msvc runner
## [0.1.0] — 2026-08-14

### Bug Fixes

- Add lib.rs, wire error module, add Yaml error variant
- *(config)* Align Default with serde defaults, propagate mkdir error, test empty-secret validation
- *(metrics)* Bounds-safe parsers, doc comments, clippy clean
- *(jobs)* Verify hostfile write, unique path, propagate MPI failure, cleanup
- *(runner)* Recover dead sessions, real disconnect via task abort
- *(tui)* Cap logs/run_output/mpi_output growth
- RAII terminal guard, drop dead test line, clarify README

### Features

- Cluster config load/save with validation
- Metrics parsing with fixtures
- Russh session connect + exec (russh 0.62)
- Jobs - hostfile builder, run_on, dispatch_mpi
- App state, channels, action/event types
- Background dispatcher + per-machine metrics poller
- Tui core loop, router, and machines/monitor/run/mpi/logs tabs
- Wire main, key routing, integration test, README

### Miscellaneous

- Scaffold hive project
- Ignore target/ and untrack build artifacts

### Security

- *(config)* Redact password in Auth Debug impl

