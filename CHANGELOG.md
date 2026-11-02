# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [1.0.0] - 2026-11-02

## [0.2.0] - 2026-11-02

### Added

- `release.sh`, bound to `just release <version>`: bumps `Cargo.toml`/`Cargo.lock`, turns the CHANGELOG's `[Unreleased]` section into a dated version section, runs `just check`, commits, tags `vX.Y.Z`, and (after confirmation) pushes — triggering the existing release workflow.

## [0.1.0] - 2026-11-02

### Added

- Basic CLI: read `mvn dependency:tree` output from stdin, a file (`--file`), or by running `mvn dependency:tree` itself when stdin isn't piped.
- Search mode: given a package name, find all hits (substring, case-insensitive) and show the path from the root down to each hit.
- Dump mode: without a package name, the whole dependency tree is shown.
- Coloring: the root in bold, direct dependencies in green, transitive ones in yellow. Automatically respects the `NO_COLOR` convention, and can be disabled explicitly with `--no-color`.
- Support for multi-module projects: each module in the `dependency:tree` output is treated as its own tree.
- `--include-scope` / `--exclude-scope`: filter dependencies (and their subtree) by Maven scope. An unknown scope prints an error (valid: `compile`, `provided`, `runtime`, `test`, `system`) before any input is read.
- `--ignore <PACKAGE>`: exclude hits that are a (transitive) dependency of a given package.
- `--module <NAME>`: in multi-module projects, restrict to the module(s) matching a name.
- `--search-depth` / `--depth`: separates search depth (which hits count) from display depth (how much of each path is shown, with deduplication of identical truncated paths).
- `--no-transient`: shorthand for direct dependencies only.
- `--json`: structured JSON output for both search and dump mode.
- `justfile` with `build`, `test`, `lint`, `fmt`, `check`, and `install`.
- GitHub Actions CI (`.github/workflows/ci.yml`): `fmt --check`, `clippy -D warnings`, and `test` on Linux, macOS, and Windows for every push/PR.
- GitHub Actions Release (`.github/workflows/release.yml`): pushing a `v*.*.*` tag builds and publishes binaries to GitHub Releases for Linux x86_64 (`.tar.gz` and `.AppImage`), macOS universal x86_64+arm64 (`.tar.gz`), and Windows x86_64 (`.zip`).
- Dependabot (`.github/dependabot.yml`): weekly updates for both Cargo dependencies and GitHub Actions.
