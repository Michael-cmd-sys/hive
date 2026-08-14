# hive — Cluster Management & Monitoring TUI (Design Spec)

**Date:** 2026-08-14
**Status:** Approved (design)
**Author:** opencode (for Michael-cmd-sys)

## 1. Problem

A final-year applied-math undergrad needs to distribute MPI/C workloads across several
lab PCs joined into an ad-hoc cluster. Today this means manually `ssh`-ing into each
machine (collecting passwords), and visually checking each box's CPU/RAM load — tedious
and low-value. `hive` is a Rust TUI that automates connection to a set of machines and
gives a single live dashboard of cluster load, plus the ability to run commands and
launch MPI jobs across the nodes.

## 2. Goals

- Connect to multiple machines over SSH using either a password or an SSH key.
- Manage the machine list from a YAML file **and** edit it from within the TUI.
- Show a live system monitor (CPU%, RAM used/total, load average, cores, uptime) for
  every connected node.
- Run ad-hoc shell commands on one / many / tagged nodes and view output.
- Launch an MPI job (e.g. `mpirun -hostfile ...`) across the connected nodes from the TUI.

## 3. Non-goals (YAGNI)

- No agent binary deployed to nodes (metrics come from remote shell commands).
- No Prometheus / external exporter integration.
- No scheduling/orchestration beyond a single MPI launch invocation.
- No multi-user accounts or remote API server.

## 4. Tech stack

- **Language:** Rust (edition 2021+), toolchain cargo 1.95.
- **SSH:** `russh` (pure-Rust async SSH2 client on Tokio). Password + key auth.
- **Async runtime:** Tokio.
- **TUI:** `ratatui` + `crossterm`.
- **Config/serde:** `serde`, `serde_yaml`.
- **CLI args (optional):** `clap` for `--config <path>`.
- **Errors:** `thiserror` + `anyhow` where convenient.

## 5. Project layout

```
hive/
  Cargo.toml
  src/
    main.rs            # bootstrap: load config, start tokio runtime, launch TUI
    config.rs          # ClusterConfig load/save (serde_yaml)
    ssh/
      mod.rs           # Session wrapper over russh
      auth.rs          # password vs key auth resolution
    metrics.rs         # MachineStats + remote command parsing
    jobs.rs            # command runner + MPI dispatcher (hostfile builder)
    tui/
      mod.rs           # app state, event loop, tab router
      machines.rs      # Machines tab
      monitor.rs       # Monitor tab
      run.rs           # Run tab
      mpi.rs           # MPI tab
      logs.rs          # Logs tab
      widgets.rs       # shared ratatui helpers
  tests/
    metrics_parse.rs   # unit tests for metrics parsing
    config_roundtrip.rs# unit tests for config (de)serialization
    live_smoke.rs      # integration: connect to localhost sshd if present
  docs/superpowers/specs/2026-08-14-hive-design.md
```

## 6. Configuration

File: `cluster.yaml`. Resolved in this order: `--config <path>` > `./cluster.yaml` >
`~/.config/hive/cluster.yaml`. If none exists, `hive` starts with an empty machine list
and the user adds nodes via the TUI (which then writes the file).

```yaml
poll_interval_ms: 2000
mpi:
  launcher: mpirun          # or mpiexec
  default_args: "-bind-to core"
machines:
  - name: node1
    host: 192.168.1.20
    port: 22
    user: alice
    auth:
      method: password      # password | key
      password: "secret"    # required iff method: password
      key_path: ~/.ssh/id_rsa   # required iff method: key (expanded via ~)
    tags: [gpu, lab]        # optional grouping for Run/MPI targets
```

`MachineConfig` fields are validated on load (host + user present; exactly one of
password/key_path present per method). Invalid YAML -> clear error, keep last-good in memory.

## 7. Modules

### 7.1 config
- `load(path) -> Result<ClusterConfig>`
- `save(path, &ClusterConfig) -> Result<()>`
- Field expansion: `~` in `key_path` resolved at connect time.
- Adding/removing/editing a machine in the TUI mutates the in-memory config and calls `save`.

### 7.2 ssh
- `Session { handle: russh::client::Handle, .. }`
- `connect(cfg: &MachineConfig) -> Result<Session>` — password via
  `russh::client::Auth::Password`, key via `Auth::PublicKey` with a loaded key.
- `exec(&mut self, cmd: &str) -> Result<CmdOutput>` where `CmdOutput { stdout, stderr, exit }`.
  Uses a dedicated channel per call; the underlying TCP/session is kept alive for reuse.
- On auth failure / timeout, returns `Err` without crashing the app.

### 7.3 metrics
- `MachineStats { cores, cpu_percent, mem_used_mib, mem_total_mib, load1, load5, load15, uptime }`
- `collect(session) -> Result<MachineStats>`: runs, over the session,
  - `nproc` -> cores
  - `free -m` -> mem_used/total (used = total - available, or Mem: used line)
  - `cat /proc/loadavg` -> load1/5/15
  - `mpstat 1 1` (fallback `top -bn1`) -> cpu idle -> cpu_percent = 100 - idle
  - `uptime` or `/proc/uptime` -> uptime
- Robust line parsing with unit tests (`metrics_parse.rs`); tolerate missing `mpstat`
  by falling back to `top`.

### 7.4 jobs
- `run_on(session, cmd) -> stream of CmdOutput` (buffered; full output shown in Run tab).
- `dispatch_mpi(head: &Session, workers: &[MachineConfig], binary: &str, args: &str)`:
  1. Build a hostfile string: one `host:slots` line per worker (`slots` = cores from metrics).
  2. Write hostfile to the head node (e.g. via `scp`/exec `cat > /tmp/hive_hostfile`).
  3. `exec` on head: `<launcher> -hostfile /tmp/hive_hostfile <default_args> <args> <binary>`.
  4. Stream/return output + exit code to the MPI tab.
- Note: real MPI launch assumes `mpirun`/`mpiexec` and the target binary are already
  installed on the head node and reachable on workers (e.g. shared FS or pre-copied).
  `hive` does **not** install MPI; it only assembles the hostfile + command. This
  assumption is documented in-app (MPI tab help line).

### 7.5 tui
Single Tokio task owns the ratatui terminal; an async channel receives `Event`s
(keyboard + background metric/connection updates). A background task per machine runs
`metrics::collect` every `poll_interval_ms` and sends results to the UI channel.

Tabs:
- **Machines:** list (name, host, status: disconnected/connecting/connected/error),
  add/remove/edit (form), connect/disconnect, connect-all. Edits save to YAML.
- **Monitor:** one card per connected node: CPU% (bar), RAM used/total (bar), load
  avg, cores, uptime; color-coded (green/amber/red by threshold). Sortable.
- **Run:** target selector (node / all / tag), command input, output pane (per-node or
  merged).
- **MPI:** head-node picker, worker multi-select, binary + args fields, Launch button,
  launch log + a help line stating the install assumption.
- **Logs:** rolling log of connection/error/info events.

Keyboard: Tab/BackTab to switch tabs; standard field navigation; `q` quits (confirms if
jobs running).

## 8. Data flow

```
main.rs
  ├─ load ClusterConfig
  ├─ spawn tokio tasks: one metrics-poller per machine (timer -> ssh::exec -> metrics::collect -> send StatsUpdate)
  ├─ spawn ssh connection tasks on demand (Machines tab)
  └─ TUI event loop (ratatui): renders state, sends user actions (connect, run, mpi) to a command channel
                                                                              │
                                              ssh/jobs tasks execute actions, send results/Logs back to UI channel
```

All cross-thread communication via `tokio::sync::mpsc`. The TUI holds `AppState`
(locked `Arc<Mutex<...>>` or message-driven) with `machines: Vec<MachineView>`,
`stats: HashMap<name, MachineStats>`, `logs: Vec<LogEntry>`.

## 9. Error handling

- `ssh::connect`/`exec` and `metrics::collect` return `Result`; a single node failure
  updates that node's status to `error(reason)` and logs it — never panics the app.
- YAML load errors are reported clearly; last-good config retained.
- Passwords are **never** written to logs.

## 10. Security

- Passwords are stored in plaintext YAML (explicit user requirement). Mitigations:
  - Masked in the TUI (shown as `••••••`).
  - Not logged.
  - File written with `0600` permissions when created by the app.
  - Documented risk in README; optional follow-up: env-var or OS keyring for secrets.

## 11. Testing

- `tests/metrics_parse.rs`: parsing of `free -m`, `/proc/loadavg`, `mpstat`, `nproc`
  with sample fixtures (incl. `mpstat`-absent fallback path).
- `tests/config_roundtrip.rs`: serialize -> deserialize preserves machines/auth; invalid
  configs rejected.
- `tests/live_smoke.rs`: if `sshd` is reachable on `localhost` (or `$HIVE_TEST_SSH` set),
  connect + collect metrics + run `echo`. Skipped otherwise ( `#[ignore]`-style guard).

## 12. Open assumptions (recorded per user request)

1. MPI binaries (`mpirun`/`mpiexec`) and the user's compiled C/MPI program are already
   present on the head/worker nodes; `hive` only builds the hostfile + invokes the launcher.
2. Lab nodes run Linux with `bash`, `free`, `nproc`, `/proc`. `mpstat` optional.
3. Network: the controlling machine can reach each node's SSH port; credentials supplied
   are valid.

## 13. Build / run

```
cd hive
cargo run -- --config ./cluster.yaml
# or just: cargo run   (uses ./cluster.yaml or ~/.config/hive/cluster.yaml)
```
