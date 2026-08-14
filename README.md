# hive

> SSH cluster orchestration for the swarm — a fast terminal UI to manage, monitor, and command a lab cluster from one place.

`hive` is a Rust + [ratatui](https://github.com/ratatui-rs/ratatui) terminal application for
scientific-computing clusters. It connects to many machines over SSH (password **or** key),
shows live per-node CPU / memory / load, runs shell commands across the fleet, and launches
MPI jobs — all from a single keyboard-driven界面.

---

## Features

- **Machines** — add, connect, delete, or wipe cluster nodes interactively (no YAML editing required).
- **Live monitoring** — per-node CPU busy %, RAM, load average, core count, and uptime, polled on an interval.
- **Ad-hoc commands** — type a command once and run it on every node; output streams back into the TUI.
- **MPI dispatch** — launch `mpirun` jobs across the cluster with a generated `HOSTFILE`.
- **Secure by default** — passwords are kept **only in memory** for the life of the process and are
  never written to disk; config files are created `0600`.
- **Zero agent** — uses plain `ssh`/shell commands on the remote side; nothing to install on the nodes.

---

## Installation

### From source

```bash
git clone https://github.com/Michael-cmd-sys/hive
cd hive
cargo build --release
# binary: target/release/hive
```

### With cargo

```bash
cargo install --path .
```

### Prebuilt binaries

Production binaries for **Linux (gnu + musl)**, **Windows**, and **macOS** are produced by CI:

- `rolling` — latest build from `main` (prerelease).
- `vX.Y.Z` — tagged stable releases.

Grab `hive-<target>` from the **Releases** page.

---

## Quick start

```bash
# run with a config next to the binary (or pass --config)
cargo run -- --config ./cluster.yaml
```

With **no config**, the Machines tab shows an onboarding hint. Press **`a`** to add your first
node interactively, or drop a `cluster.yaml` next to the binary and press **`c`** to connect to all.

On launch you are greeted by the **hive** banner on the Logs tab.

---

## Configuration

`hive` reads a YAML file (default `cluster.yaml`, override with `--config <path>`). Example:

```yaml
poll_interval_ms: 2000
mpi:
  launcher: mpirun
  default_args: "-bind-to core"
machines:
  - name: node1
    host: 192.168.1.20
    port: 22
    user: alice
    auth:
      method: password        # secret is NOT stored; you are prompted at connect time
    tags: [gpu, lab]
  - name: node2
    host: node2.local
    user: bob
    auth:
      method: key
      key_path: ~/.ssh/id_ed25519
```

| Field           | Meaning                                                            |
| --------------- | ----------------------------------------------------------------- |
| `name`          | Friendly label shown in the UI.                                   |
| `host`          | IP or hostname (resolvable from the machine running `hive`).     |
| `port`          | SSH port (default `22`).                                          |
| `user`          | Remote SSH user.                                                  |
| `auth.method`   | `password` or `key`.                                              |
| `auth.key_path` | Path to a private key (expanded via `~`); used when `method: key`.|

> **Passwords are never persisted.** A `password` machine stores only the method; the secret is
> collected in-memory when you add the node and re-prompted on connect after a restart.

---

## Usage

Global keys: **`Tab`** / **`←` `→`** / **`h` `l`** switch tabs · **`c`** connect to all · **`s`** save config · **`q`** quit.

**Machines**

| Key            | Action                                                            |
| -------------- | ----------------------------------------------------------------- |
| `a`            | Add a machine interactively (name → host → ssh user → auth).     |
| `↑`/`↓` or `j`/`k` | Select a machine.                                            |
| `Enter`        | Connect to the selected machine (prompts for a password if needed).|
| `d`            | Delete the selected machine (updates `cluster.yaml`).            |
| `D`            | **Nuclear wipe** — erase *every* machine. Confirm with `y` (`n`/`Esc` cancels).|
| `c`            | Connect to all machines in the config.                           |

**Run**

| Key     | Action                                                            |
| ------- | ----------------------------------------------------------------- |
| `Enter` | Type a shell command, then `Enter` again to submit it.           |
| `t`     | Toggle command scope: **all nodes** ⇄ **selected machine** (shown in the prompt). |
| `r`     | Quick re-run `uname -a` on the current scope.                     |

**MPI**

| Key     | Action                                                            |
| ------- | ----------------------------------------------------------------- |
| `Enter` | Type `binary args` (e.g. `./app -n 4`), then `Enter` to launch.  |
| `t`     | Toggle job scope: **all nodes** ⇄ **selected machine**.          |
| `m`     | Launch a sample job (`hostname`) on the current scope.           |

> **Scope is visible, never hidden.** The input prompt always names the target
> (`Run on all nodes:` vs `Run on node3:`), and `t` flips between the whole fleet
> and just the machine highlighted in the Machines tab. This keeps single-node
> runs (e.g. a local box) deliberate rather than accidental.

While typing, **`Esc`** cancels. Password and key-path inputs are masked. Required fields are
validated — empty input is rejected with an on-screen error instead of being submitted.

---

## Architecture

```
┌────────────┐   Actions    ┌──────────────────┐   SSH    ┌──────────────┐
│   TUI      │ ───────────▶ │   dispatcher     │ ───────▶ │  cluster    │
│ (ratatui)  │ ◀─────────── │   (tokio task)   │ ◀─────── │  nodes      │
└────────────┘   UiEvents   └──────────────────┘          └──────────────┘
```

- `src/tui` — rendering and key handling (one module per tab + `banner`).
- `src/runner.rs` — async dispatcher that owns SSH sessions, polling, and the in-memory secret store.
- `src/ssh.rs` — `russh`-based session wrapper.
- `src/metrics.rs` — parse `nproc`/`free`/`mpstat`/`loadavg`/`uptime` into stats.
- `src/config.rs` — `ClusterConfig` (de)serialization, `0600` on write.
- `src/jobs.rs` — ad-hoc command runner and MPI `HOSTFILE`/dispatch.

Stats are gathered with ordinary remote shell commands, so no agent binary is required on the
nodes.

---

## Security model

- **Passwords are in-memory only.** `Auth::Password` stores no secret; the password lives in the
  dispatcher's `secrets` map for the process lifetime and is re-prompted after a restart.
- **Config files are `0600`.** Written with owner-only permissions where the platform allows.
- **SSH host-key verification is disabled** (`check_server_key` always returns `true`). This is
  intentional for trusted lab networks and is **not** suitable for untrusted networks — prefer SSH
  keys and a proper `known_hosts` check before any production use.

---

## Development

```bash
cargo build                 # debug build
cargo test                  # unit + integration tests
cargo clippy --all-targets  # lints
cargo run -- --config dev.yaml
```

Integration tests live under `tests/`; `tests/live_smoke.rs` is `#[ignore]` and requires
`HIVE_TEST_SSH` + `HIVE_TEST_PW` to run against a real host.

---

## Releases & versioning

This repo uses [Conventional Commits](https://www.conventionalcommits.org) +
[git-cliff](https://github.com/orhun/git-cliff) + [cargo-release](https://github.com/crate-ci/cargo-release).

- **`CHANGELOG.md`** is regenerated automatically on every push to `main`
  (`.github/workflows/changelog.yml`).
- **Push to `main`** builds binaries for Linux (gnu + musl), Windows, and macOS and publishes them
  as a rolling prerelease tagged `rolling` (`.github/workflows/release.yml`).
- **Pushing a `vX.Y.Z` tag** builds the same binaries and publishes a GitHub Release with the
  generated changelog.

Cut a release:

```bash
cargo install cargo-release git-cliff   # one-time
cargo release patch --execute           # 0.1.0 -> 0.1.1, tags v0.1.1
git push --tags                         # triggers the versioned release
```

---

## Contributing

Issues and PRs welcome. Please follow Conventional Commits so the changelog stays meaningful, and
run `cargo clippy --all-targets` before opening a PR.

---

## License

MIT — see `LICENSE`.
