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
Passwords are stored in plaintext YAML (per the project requirement). The app writes the file
with 0600 permissions, redacts secrets in `Debug` output and logs, and skips SSH host-key
verification (lab-only, not for production). Prefer SSH keys where possible.

## Releases & versioning

This repo uses [Conventional Commits](https://www.conventionalcommits.org) + [git-cliff](https://github.com/orhun/git-cliff) + [cargo-release](https://github.com/crate-ci/cargo-release).

- **CHANGELOG.md** is regenerated automatically on every push to `main` (see `.github/workflows/changelog.yml`).
- **Push to `main`** builds production binaries for Linux (gnu + musl), Windows, and macOS and
  publishes them as a rolling **prerelease** named `rolling` (see `.github/workflows/release.yml`).
- **Pushing a version tag** (`vX.Y.Z`) builds the same binaries and publishes a proper
  GitHub Release with the generated changelog as its notes.

### Cut a release

```bash
cargo install cargo-release git-cliff   # one-time
cargo release patch --execute           # bumps 0.1.0 -> 0.1.1, tags v0.1.1, pushes
git push --tags                         # triggers the versioned GitHub Release
```

### Download a binary

Go to **Releases** on GitHub: `rolling` for the latest CI build, or a `vX.Y.Z` tag for a
versioned release. Binaries are named `hive-<target>` (e.g. `hive-x86_64-unknown-linux-gnu`).

