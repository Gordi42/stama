# Changelog

## [1.1.1] - 2026-07-10

### Fixed
- Release pipeline: static musl binaries build again (aarch64 musl toolchain), one failing target no longer cancels the others, and the release description is generated from this changelog
- Removed the CPU efficiency gauge (sacct's TotalCPU is only populated for finished steps on many clusters, so it always showed 0% for running jobs) and the emoji on the pending-reason line

## [1.1.0] - 2026-07-10

A full overhaul: stability, testing, CI/CD, and new features.

### Added
- **Pending-reason explanations**: pending jobs show the Slurm reason code with a plain-English explanation in the details pane
- **Efficiency gauges**: memory and time-limit usage bars (from sacct) for the selected job
- **Live log view** (`L`): fullscreen viewer with follow mode (`f`/`G`), scrollback, and search (`/`, `n`/`N`)
- **Job filter** (`f`): live, case-insensitive filtering across the displayed columns (regex supported); display-only
- **Configurable columns**: `job_columns` option selects and orders the table columns (id, name, status, time, partition, nodes, priority, reason, account, qos, cpus, nodelist)
- **Job-array grouping**: array tasks collapse into one expandable row (Space) with aggregate state counts; killing a group cancels the whole array
- **Node picker**: ssh-to-node on multi-node jobs opens a node-selection popup
- **Notifications** (opt-in): terminal bell and OSC 777 desktop notifications when jobs start or finish
- **Shell completions**: `stama --completions bash|zsh|fish`
- Prebuilt static binaries (x86_64/aarch64, musl + gnu) attached to GitHub releases

### Fixed
- Slurm errors now show an error popup instead of a silently empty job list; a hung squeue recovers after 30 s
- Crash fixes: malformed squeue output, narrow terminals, the salloc "Create new" row, missing editor binaries
- The terminal is restored on errors, panics, and SIGTERM/SIGINT/SIGHUP
- Corrupt or older config files are never silently overwritten; missing options fall back to defaults
- `sacct` no longer receives squeue-specific arguments; job ids sort numerically; long error messages scroll
- `stama --help` exits without starting the TUI

### Changed
- Dependencies modernized: ratatui 0.30, crossterm 0.29, ratatui-textarea 0.9, toml 0.9; MSRV is now Rust 1.88
- Recommended install: prebuilt musl binary, or `cargo install --locked stama`

## [1.0.2] - 2024

Previous release.
