# stama — Improvement Research Report

*Compiled 2026-07-10 from five parallel deep-research passes: architecture review, bug hunt, testing strategy, CI/CD + dependency audit, and feature/competitor research. All code claims were verified against the actual source; example tests were compiled and run; the parser panic was reproduced.*

---

## Executive summary

The bones are good: the Action-enum event flow, the panic hook that restores the terminal, the background-updater concept, and the 12 existing tests put stama ahead of most learning projects. The "surprisingly stable" daily experience is real — but only because the happy path (default squeue command, existing presets, a large terminal, an installed editor) threads between every crash below. Each one is exactly one config edit, one Tab keypress, or one window resize away.

The four structural problems:

1. **Confirmed crash paths** — the worst being a reproducible panic in the squeue parser (a commented-out bounds check) that kills the background updater and freezes the job list forever.
2. **Errors swallowed as sentinel strings** — a broken/down Slurm looks identical to "no jobs". Systemic in `update_content.rs`.
3. **~700 lines of copy-paste** across the seven menu modules because there is no `Menu` trait, and Slurm command execution scattered through UI code with no abstraction (which is also why the code is untestable).
4. **No CI, dated dependencies** — two crossterm versions compiled into the binary, `toml 0.5` (unsupported since 2022, has a known serialization bug in the config-save path), a manifest that legally permits a RUSTSEC-affected regex, and 81–90 clippy warnings.

---

## Part 1 — Confirmed bugs (ranked)

### Crashes

| # | Bug | Location | Trigger |
|---|-----|----------|---------|
| 1 | **Unguarded `parts[0..9]` indexing in squeue parser; guard commented out** (`// if parts.len() < 11` — it was off-by-one, should be `< 10`) | `src/update_content.rs:255-273` | Any squeue stdout line with <10 fields: `squeue --help` as the user command, `-M <cluster>` (Slurm prepends a `CLUSTER:` banner), site wrappers printing banners. **Reproduced.** Compound damage: the panic fires in the worker thread → the global panic hook resets the terminal *while the main loop keeps drawing* (garbled screen), and `ContentUpdater::tick` treats `Disconnected` like `Empty` (`update_content.rs:58-64`) so the job list **silently freezes forever**. |
| 2 | **Index OOB in salloc menu with "Create new" selected** — `set_entry` writes `entries[index]` where `index == len` is legal | `src/menus/salloc/salloc_menu.rs:161` | Fresh install: `a` → Tab → any key. Two keypresses. |
| 3 | **Squeue-command textarea rendered into unclipped Rect** — `Buffer::index_of` panics for out-of-area cells (verified against pinned ratatui 0.29 / tui-textarea 0.4 sources) | `src/menus/job_overview.rs:224-229` | Terminal narrower than ~12 + command length; default command needs ~30 cols. Fix: `squeue_rect.intersection(*area)`. |
| 4 | **Missing editor binary panics** — `.spawn().expect(...)` on free-form user config | `src/app.rs:335-345` | Typo in `external_editor` or editor not on cluster PATH. `start_salloc` (app.rs:448) already does this right — copy that pattern. |
| 5 | **Duplicate-job dedup removes by stale indices** — collects indexes then `remove(index)` one by one; each removal shifts later indexes | `src/update_content.rs:155-171` | ≥3 rows sharing a JobID (CG row + two sacct rows, e.g. requeued jobs). Panics if last duplicate was final element; otherwise removes the wrong job. Fix: `retain(...)` or remove in reverse. |
| 6 | Smaller panic paths | `salloc_menu.rs:440` (u16 underflow on border click, debug builds), `event.rs:99-110` (`unimplemented!()` for FocusGained/Paste events; `expect` on poll/read), `event.rs:77` (`send().unwrap()`), `write_output.rs:15` (unwritable `-o` path), `tui.rs:47` (panic-in-panic-hook → abort; use `let _ = Self::reset()`) | |

### Terminal state

7. **Error returns don't restore the terminal** — `main.rs:36-63`: any `Err` from `tui.draw()?` / `events.next()?` propagates out without `tui.exit()`; shell left in raw mode + alternate screen, eyre report invisible. Fix: `Drop` impl for `Tui` or wrap the loop.
8. Panic hook re-registered (nested) on every editor/salloc round trip — `tui.rs:45-49`. Cosmetic; register once in `main`.

### Concurrency

9. **One hung squeue freezes all future refreshes silently** — single process slot, blocking `recv()` with no timeout (`update_content.rs:55-71,129`). Data goes stale with no indication. Fix: respawn after N seconds; show "last updated" staleness.
10. **scancel/squeue run on the UI thread** — `app.rs:242` (kill_job), `app.rs:395` (ssh_to_node): `Command::output()` blocks the event loop; slow slurmctld freezes the UI mid-keystroke.
11. **Old event thread lingers up to one tick** after `stop()` (`event.rs:75-80` + `main.rs:53-62`) and can steal the first keystrokes meant for vim/salloc. Fix: keep the `JoinHandle` and join.
12. **refresh_rate = 0 busy-spins** — settable via the options menu; `poll(0)` + immediate Tick pegs a CPU. Clamp to a minimum (~50 ms).

### Parsing / errors

13. **Slurm errors swallowed into an empty job list** — on non-zero exit, `"Error executing command"` is returned *as data* (`update_content.rs:240-249,317-326`), parsed to zero jobs. Slurm down = "No jobs found". stderr never shown.
14. **Header skipped blindly by position** (`skip(1)`, `update_content.rs:254`) — user's `--noheader` silently drops the first job every refresh; `-M` shifts everything.
15. **sacct is fed squeue's arguments** (`update_content.rs:295-315`) — flags mean different things (`-p` = partition in squeue, "parsable" in sacct); errors → empty completed list, swallowed per #13.
16. Delimiter `|%|` collision in job names shifts all columns; names truncated at 32 chars quietly break `%x` log-path substitution (`job.rs:115-146`).
17. `whoami` output not trimmed (`joblist.rs:87-96`); on failure the literal string "Error executing whoami" becomes the username in the squeue command. Read `$USER` instead.

### Command splitting / escaping

18. Naive `split_whitespace` everywhere (`app.rs:331,436`; `update_content.rs:234,296`; `salloc_entry.rs:44-70`): editor paths with spaces break (and panic per #4); preset names with spaces produce mangled salloc args; `cd {workdir}` / `ssh {node}` written **unquoted** to the output file the shell wrapper evals — a workdir with spaces breaks, one with `;`/`$( )` executes as shell code. Fix: `shell-words` crate + quote the exit command.

### Config

19. **Any config error silently resets AND overwrites the user's config** — no `#[serde(default)]` on `UserOptions`, so one new field makes *all* old configs unparsable → silent fallback to defaults → save-on-deactivate destroys the old file (`user_options.rs:41-49` + `user_options_menu.rs:116-120`; same pattern for salloc presets at `salloc_menu.rs:52,84`). `save()` errors are swallowed too.

### State desync

20. When the selected job vanishes between refreshes, whatever job now occupies that index becomes selected without resetting the details pane (`joblist.rs:269-278`). Display-level only (scancel uses a cloned Job).
21. Deleting the first salloc preset wraps selection to "Create new" instead of the new first entry (`salloc_menu.rs:190`).
22. **Lexicographic job-id sort** — `joblist.rs:320` sorts `String` ids, so job "9" sorts after "10". User-visible.

---

## Part 2 — Architecture & code quality

### The two structural refactors

**A. `Menu` trait + menu stack** (deletes ~600-800 lines). Every one of the 7 menus hand-rolls the same five blocks: centered-popup layout (6 near-identical copies), wrap-around `set_index` (4 copies), next/previous, close-on-outside-click, and the `if !self.handle_input { return false }` guard. `MenuContainer` (`menus.rs:159-210`) routes input through 7 hardcoded if-cascades — and the **key order differs from the mouse order** (confirmation-first vs message-first), so with both open, keys go to one menu and clicks to the other. The twin `should_render`/`handle_input` bools on every menu are really "is on the stack".

```rust
pub trait Menu {
    fn render(&mut self, f: &mut Frame, area: Rect, ctx: &AppContext);
    fn input(&mut self, key: KeyEvent) -> InputResult;  // Ignored | Consumed | Act(Action) | Close
    fn mouse(&mut self, m: &mut MouseInput) -> InputResult { InputResult::Ignored }
}
pub struct MenuStack { stack: Vec<Box<dyn Menu>> }  // one loop replaces 7 ifs, same order for key & mouse
```

**B. `Scheduler` trait for all Slurm calls** (unlocks testing, fixes silent errors by construction). Nine `std::process::Command` call sites are scattered across app.rs (scancel, squeue-for-ssh, editor, salloc), joblist.rs (whoami), and update_content.rs (squeue, sacct, scontrol, tail):

```rust
pub trait Scheduler: Send + Sync {
    fn queued_jobs(&self, filter: &SqueueFilter) -> Result<Vec<Job>>;
    fn finished_jobs(&self, user: &str) -> Result<Vec<Job>>;
    fn job_details(&self, id: &JobId) -> Result<String>;
    fn cancel(&self, id: &JobId) -> Result<()>;
    fn nodes_of(&self, id: &JobId) -> Result<Vec<String>>;
}
```

`App` holds `Arc<dyn Scheduler>`; tests inject a fake. `Result` errors route to the existing `Message` popup (the pattern `kill_job` and `ssh_to_node` already use correctly). This is also the one place to fix sacct-inherits-squeue-args and the parsing guards.

### Smaller refactors, by impact

- **Simplify `update_content.rs`**: the 4-channel/4-thread fan-out inside `get_content` (with dummy `thread::spawn(|| {})` placeholders) buys nothing — the whole function already runs on a background thread. One sequential worker, `Result` all the way, fixes 20 unwraps at once.
- **Type the command pipeline**: replace `squeue_command: String`, `Action::StartSalloc(String)`, `exit_command: Option<String>` with argv vectors / `enum ExitCommand { Cd(PathBuf), Ssh(String) }`; delete the four `split_whitespace` re-parsers.
- **Named fields instead of positional `entries[0..4]`** extraction (`user_options_menu.rs:86-109`, `entry_menu.rs:99-142` — where a type mismatch makes the string `"error"` a real config value).
- **Move clap to startup**: `write_output.rs` builds the CLI parser *after the TUI exits* — `stama --help` runs the entire TUI first.
- **`JobId` newtype** with numeric-aware `Ord` (handles `123_4` array syntax) fixing the sort bug.
- **Dead code**: `OpenMenu::JobOverview` + stub, `set_index_raw`, `JobList::sort` (only used by tests), `SallocList::{get_mut,set_list}`, unjoined `MyProcess.handler`, `EntryMenu.is_new`, 12-line commented-out block in `get_log_tail`.
- `TextField::input` hardcodes `Action::UpdateUserOptions` on Enter (`text_field.rs:215`) — a generic widget knows about one use case; editing a salloc field fires a user-options update.
- ~81–90 clippy warnings, ~72 auto-fixable (`cargo clippy --fix`): redundant field names, `.clone()` on `Copy` Rects, single-wildcard-arm matches, `i32` indexes with casts.

---

## Part 3 — Testing strategy

Not starting from zero: 12 tests pass (joblist navigation/sorting, salloc TOML round-trip, format_time). But the riskiest code — the parsers and everything behind `Command` — is untested. Note: **binary-only crate** (no lib.rs), so new tests go in `#[cfg(test)]` modules inside source files; adding a lib target is a later nicety. Also: the existing salloc_list tests **write into the real `~/.config/stama/`** on `cargo test` — fix with `tempfile` + injected base dir.

### Week 1 (~2-3 h, 80% of the value) — pure functions, no refactoring

Priority targets (example tests were compiled and verified against current code by the research agent — see agent transcript):

| Target | Location | Why |
|---|---|---|
| `format_sacct_output` | `update_content.rs:329` | Feeds the whole UI; step/RUNNING filtering |
| `format_squeue_output` | `update_content.rs:252` | The confirmed panic |
| `format_time_used` / `format_time_pending` | `update_content.rs:418,430` | Pure, trivial |
| `Job::get_stdout` | `job.rs:115` | `%j`/`%x`/`%<n>j` expansion; wrong = "log not found" |
| `SallocEntry::start` | `salloc_entry.rs:44` | Pure command builder |
| sort categories + `UserOptions` TOML round-trip | `joblist.rs:311`, `user_options.rs` | Catches config-breaking field renames |

Immediately after: restore the length guard as `if parts.len() < 10 { continue; }` with a `malformed_input_does_not_panic` regression test.

### Stage 2 — inject Slurm, snapshot the UI (~1 day, spread out)

- 4-method `SlurmClient`/`Scheduler` trait (Part 2B); hand-rolled `FakeSlurm` with canned fixtures — **skip mockall**. Capture real fixtures from your cluster once and sanitize.
- `JobOverview::render` already takes plain data — ratatui `TestBackend` tests work **today with zero new dependencies** (substring asserts for behavior). Add `insta` snapshots for exactly the 4 collapsed/extended layout states + empty joblist; `TestBackend` implements `Display` in ratatui 0.29 so `insta::assert_snapshot!(terminal.backend())` just works.
- `FakeSlurm`-driven `App` tests: `App::input`/`handle_action` are already callable without a terminal — "pressing kill on job X scancels X" without a cluster.

### Later / targeted

- **proptest** (over quickcheck) for exactly two properties: parsers never panic on arbitrary bytes (`"\\PC*"` input — currently fails!), and JobList selection invariants under random action sequences.
- **Skip**: `assert_cmd` (no non-TUI CLI surface), PTY testing as a suite (at most one `#[ignore]`d smoke test run before releases), `criterion`, `rstest`.

Dev-dependencies worth adding: `insta`, `proptest`, `pretty_assertions`, `tempfile`.

---

## Part 4 — Dependencies & CI/CD

### Dependency audit (verified against crates.io, 2026-07)

- **Two crossterm versions in the binary** (confirmed via `cargo tree -d`): 0.27 (stama + tui-textarea 0.4) and 0.28.1 (ratatui 0.29), plus duplicate mio and unicode-width. Fix: `crossterm = "0.28"` + `tui-textarea = "0.7"` (0.7 targets exactly ratatui 0.29 + crossterm 0.28).
- **toml 0.5**: unsupported since 2022; the 0.5 line's `TableAfterValue` serialization errors are a latent bug in the config-save path. `toml = "0.9"` (or `"1"`) — stama only uses `from_str`/`to_string`, both unchanged. Requires Rust ≥ 1.85 → declare `rust-version = "1.85"`.
- **regex req `1.5.4`** is affected by RUSTSEC-2022-0013 (ReDoS); lockfile resolves safely to 1.11.1, but bump the written requirement to `"1.11"`.
- **ratatui 0.30** exists (workspace split, drops the unmaintained-`paste` advisory) but is a real migration and tui-textarea 0.7 doesn't support it — do later, with a maintained textarea fork (`tui-textarea-2` / `ratatui-textarea`).
- Build health: `cargo check` clean, `cargo fmt --check` clean, `cargo clippy` 81 warnings (72 auto-fixable). **A `-D warnings` gate fails until the clippy sweep lands.**
- No exploitable RUSTSEC advisories in the locked tree; `cargo audit` will warn (not fail) about `paste` via ratatui 0.29.

### CI (`.github/workflows/ci.yml`)

Jobs: test + `clippy -D warnings` + `fmt --check` (stable, `Swatinem/rust-cache@v2`), an MSRV check pinned to `rust-version`, and `cargo audit` via `taiki-e/install-action`, on push/PR + weekly cron. Full ready-to-commit YAML is in the CI/CD agent report (concurrency-cancel, `dtolnay/rust-toolchain`).

### Release (`.github/workflows/release.yml`)

**cargo-dist evaluated and not recommended** (alive again post-Axo-shutdown — features merged back as of v0.29, releases through 2026 — but its value is installers/multi-OS; stama needs Linux-only tarballs). Hand-rolled ~70-line workflow with `taiki-e/create-gh-release-action` + `taiki-e/upload-rust-binary-action`:

- Targets: `x86_64-unknown-linux-{gnu,musl}` + `aarch64-unknown-linux-{gnu,musl}` (free `ubuntu-22.04-arm` runners, no cross needed). Build on ubuntu-22.04, not -latest, for older glibc. **The musl static binary is the headline artifact — HPC login nodes run ancient glibc and users have no root; this is the single biggest adoption unlock.**
- crates.io publish via **Trusted Publishing** (OIDC, no stored token): configure at crates.io → stama → Settings → Trusted Publishing, `release` environment.

### Supporting config

Worth it: grouped Dependabot (one weekly cargo PR + monthly actions), a minimal `[lints]` table in Cargo.toml, `rust-version`. Skip: `rust-toolchain.toml`, `.rustfmt.toml` (already clean under defaults), cargo-deny, Renovate.

---

## Part 5 — Features (from competitor + pain-point research)

Landscape: **turm** (Rust, the direct competitor — inotify log-following is its headline), **stui** (Go "k9s for Slurm" — nodes/sacct/sdiag views, multi-select bulk actions, static-binary installer), **SlurmCommander** (Go, dead since 2023 — cautionary tale: broke when Slurm 23 changed output formats), reportseff/seff (efficiency CLIs). stama's GitHub issues: zero filed; notes.md contains no unshipped ideas — the roadmap must come from the ecosystem. turm's issue tracker is the best proxy for what stama's audience wants: configurable keybinds, `--start` estimated start, column selection, **compact job arrays**, non-UTF-8 log tolerance, packaging.

Top 5 for a solo maintainer (suggested order 1 → 3 → 4 → 2 → 5):

1. **Pending-job insight** ★ — Reason column + plain-English explanation of Slurm reason codes (`Priority` → "higher-priority jobs are ahead of you", `QOSMaxCpuPerUserLimit` → "you hit your QOS CPU cap"…) + `squeue --start` estimate. "Why is my job pending?" is *the* most common Slurm question; **no TUI answers it in words**. Difficulty: low (one more squeue field + static map + one call in the existing per-job fan-out).
2. **seff-style efficiency gauges** — CPU eff (TotalCPU/(Elapsed×AllocCPUS)), memory eff (MaxRSS/ReqMem), elapsed vs limit as colored `Gauge`s in the details pane, from sacct fields. Users over-request chronically and only learn from `seff` after the fact. No TUI has it inline. Difficulty: low-medium.
3. **musl binaries + shell completions + packaged wrapper** — `clap_complete` (clap already a dep), `stama shell-init bash` subcommand instead of README copy-paste, release workflow from Part 4. Adoption, not features. Difficulty: low.
4. **Job state-change notifications** — diff statuses on each refresh tick; terminal bell / OSC 777 desktop notification (kitty, foot, WezTerm, Ghostty) / configurable hook command. Users leave stama in a tmux pane for hours; no competitor has this. Difficulty: low-medium.
5. **Job array grouping** — collapse `12345_1..50` into one expandable row with state counts ("37 done / 3 running / 10 pending"). Most-complained-about gap in comparable tools. Difficulty: medium (ID parsing, grouping layer, tree rendering vs flat-row assumptions).

Tier 2: live log follow-mode with offset-remembering reads (turm's headline), sinfo partition/node tab, sacct history browsing with time ranges, sbatch-template submission (reuse the salloc preset infra), configurable columns + regex filter, proper multi-cluster `-M` support. Defer: configurable keybindings (high effort at current user count), SSH remote clusters.

Defensive note: SlurmCommander died on a Slurm output-format change. Keep expanding fixture tests, prefer explicit `--Format`/`--parsable2` fields (squeue currently still uses the width-padded `--Format` hack — see bug #16), and consider Slurm's `--json` output as an optional fast-path with fallback.

---

## Recommended overall sequence

**Phase 0 — a weekend of de-risking (no design decisions needed)**
1. `cargo clippy --fix` + hand-fix the remainder (~9).
2. Fix the top crash/corruption bugs: parser length guard (#1), `Disconnected` handling so updates recover (#1b), salloc `set_entry` bounds (#2), `squeue_rect` clipping (#3), editor `spawn().expect` (#4), `unimplemented!()` in event.rs, terminal reset on `Err` (#7).
3. Bump `regex = "1.11"`.
4. Week-1 unit tests (Part 3) — the agent-verified test modules drop in as-is.

**Phase 1 — infrastructure**
5. Dependency PR: crossterm 0.28, tui-textarea 0.7, toml 0.9, `rust-version = "1.85"`; verify `cargo tree -d` is clean; smoke-test mouse + textarea.
6. Commit `ci.yml` + `dependabot.yml`.
7. Set up crates.io Trusted Publishing, commit `release.yml`, cut v1.0.3, test the musl binary on your oldest cluster login node.

**Phase 2 — the two structural refactors (each enables the next)**
8. `Scheduler` trait: centralize the 9 command sites, `Result` errors → Message popup (kills the silent-empty-joblist class), fix sacct-args and dedup bugs, add `FakeSlurm` tests.
9. Simplify `update_content.rs` (drop inner threads/channels), add staleness indicator + respawn-on-hang.
10. Config hardening: `#[serde(default)]`, never overwrite an unparsable file, surface save errors.
11. `Menu` trait + stack (biggest diff — do it after the command layer stops touching `App`), then insta snapshots.

**Phase 3 — features**
12. Pending-reason explanations → musl/completions polish → notifications → efficiency gauges → array grouping.
