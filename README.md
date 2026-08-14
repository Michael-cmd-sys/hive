# hive

A Rust + ratatui TUI to SSH into multiple lab machines (password or SSH key),
monitor their CPU/RAM/load live, run commands, and launch MPI jobs across the cluster.

## Build
cargo build --release

## Config (cluster.yaml)
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
      method: password
      password: "secret"
    tags: [gpu, lab]

## Run
cargo run -- --config ./cluster.yaml

## Keys
- Tab: switch tabs (Machines / Monitor / Run / MPI / Logs)
- c: connect to all machines
- r: run `uname -a` on all nodes (sample)
- m: launch a sample MPI job across all nodes (assumes mpirun + binary preinstalled)
- q: quit

## Security
Passwords are stored in plaintext YAML (per requirement). The app writes the file
with 0600 permissions, redacts secrets in logs, and never transmits host keys
verification is skipped (lab-only). Prefer SSH keys where possible.
