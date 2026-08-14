use dioxus::prelude::*;

const CSS: &str = r#"
:root {
  color-scheme: dark;
  --bg: #04070a;
  --panel: #07120f;
  --grid: #0c2a20;
  --fg: #6cffb0;
  --fg-dim: #2f6b52;
  --amber: #ffb000;
  --red: #ff5c5c;
  --line: #12463a;
}
* { box-sizing: border-box; }
html, body { margin: 0; padding: 0; }
body {
  background: var(--bg);
  background-image:
    linear-gradient(var(--grid) 1px, transparent 1px),
    linear-gradient(90deg, var(--grid) 1px, transparent 1px);
  background-size: 28px 28px;
  color: var(--fg);
  font-family: "IBM Plex Mono", "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 15px;
  line-height: 1.55;
}
/* CRT scanline + flicker overlay */
body::after {
  content: "";
  position: fixed;
  inset: 0;
  pointer-events: none;
  background: repeating-linear-gradient(
    to bottom,
    rgba(0,0,0,0) 0px,
    rgba(0,0,0,0) 2px,
    rgba(0,0,0,0.18) 3px,
    rgba(0,0,0,0.18) 4px
  );
  z-index: 9999;
}
.wrap { max-width: 920px; margin: 0 auto; padding: 2.5rem 1.25rem 4rem; position: relative; }
.glow { text-shadow: 0 0 6px rgba(108,255,176,0.55), 0 0 18px rgba(108,255,176,0.25); }
a { color: var(--amber); text-decoration: none; border-bottom: 1px dotted var(--amber); }
a:hover { background: rgba(255,176,0,0.12); }

/* hero */
.hero { border: 1px solid var(--line); background: var(--panel); padding: 2rem 1.5rem; position: relative; }
.hero::before {
  content: "SYSTEM // hive"; position: absolute; top: -0.7rem; left: 1rem;
  background: var(--bg); padding: 0 0.5rem; color: var(--fg-dim); font-size: 12px; letter-spacing: 2px;
}
.wordmark {
  font-size: clamp(3rem, 12vw, 6rem); font-weight: 700; letter-spacing: 0.15em;
  margin: 0; line-height: 1; color: var(--fg); text-transform: lowercase;
}
.backronym { color: var(--amber); letter-spacing: 3px; font-size: 0.85rem; margin: 0.4rem 0 0; }
.tag { color: var(--fg-dim); margin: 0.8rem 0 0; }
.cursor { display: inline-block; width: 0.6em; height: 1.05em; background: var(--fg); margin-left: 2px; vertical-align: -0.18em; animation: blink 1.1s steps(1) infinite; }
@keyframes blink { 50% { opacity: 0; } }

/* telemetry grid */
.tele { margin-top: 2.5rem; }
.tele h2, .sec h2 { font-size: 0.8rem; letter-spacing: 3px; color: var(--fg-dim); text-transform: uppercase; border-bottom: 1px solid var(--line); padding-bottom: 0.35rem; margin: 0 0 0.9rem; }
.nodes { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 0.6rem; }
.node { border: 1px solid var(--line); padding: 0.55rem 0.6rem; background: #06100d; font-size: 0.78rem; }
.node .id { color: var(--fg); }
.node .bar { height: 6px; background: #0a1b16; margin: 0.4rem 0 0.3rem; border: 1px solid var(--line); }
.node .bar > i { display: block; height: 100%; background: var(--fg); }
.node .bar > i.warn { background: var(--amber); }
.node .bar > i.hot { background: var(--red); }
.node .meta { color: var(--fg-dim); }

.sec { margin-top: 2.5rem; }
.spec { border-collapse: collapse; width: 100%; font-size: 0.85rem; }
.spec td { border: 1px solid var(--line); padding: 0.5rem 0.7rem; vertical-align: top; }
.spec td:first-child { color: var(--amber); white-space: nowrap; width: 1%; }
.spec td:last-child { color: var(--fg); }

pre, .term {
  background: #020503; border: 1px solid var(--line); padding: 0.9rem 1rem;
  overflow-x: auto; color: var(--fg); font-size: 0.85rem; margin: 0;
}
.prompt { color: var(--amber); }
.cmt { color: var(--fg-dim); }

footer { margin-top: 3rem; border-top: 1px solid var(--line); padding-top: 1rem; color: var(--fg-dim); font-size: 0.8rem; display: flex; gap: 1.2rem; flex-wrap: wrap; }
"#;

// Static telemetry snapshot — evokes an HPC cluster dashboard.
const NODES: &[(&str, u8, &str)] = &[
    ("node-00", 42, "OK"),
    ("node-01", 67, "OK"),
    ("node-02", 88, "HOT"),
    ("node-03", 51, "OK"),
    ("node-04", 12, "IDLE"),
    ("node-05", 73, "OK"),
    ("node-06", 95, "HOT"),
    ("node-07", 30, "OK"),
];

fn main() {
    launch(app);
}

fn app() -> Element {
    rsx! {
        style { {CSS} }
        div { class: "wrap",
            section { class: "hero",
                h1 { class: "wordmark glow", "hive" }
                p { class: "backronym", "HETEROGENEOUS INTERCONNECTED VECTOR ENGINE" }
                p { class: "tag",
                    "cluster orchestration suite // ssh · mpi · telemetry"
                    span { class: "cursor" }
                }
            }

            section { class: "tele",
                h2 { "live cluster telemetry" }
                div { class: "nodes",
                    for (id, load, state) in NODES {
                        div { class: "node",
                            div { class: "id", "{id}" }
                            div { class: "bar",
                                i {
                                    class: if *load >= 85 { "hot" } else if *load >= 70 { "warn" } else { "" },
                                    width: "{load}%",
                                }
                            }
                            div { class: "meta", "load {load}% · {state}" }
                        }
                    }
                }
            }

            section { class: "sec",
                h2 { "specifications" }
                table { class: "spec",
                    tbody {
                        tr { td { "TRANSPORT" } td { "libssh2 over authenticated SSH; passwords captured at runtime, held in memory, never persisted to disk" } }
                        tr { td { "DISPATCH" } td { "ad-hoc shell across all or selected nodes; MPI jobs via mpirun with hostfile + per-node process count" } }
                        tr { td { "TELEMETRY" } td { "per-node CPU / RAM gauges, load average, and top processes sampled every 2s" } }
                        tr { td { "UI" } td { "ratatui TUI — tabbed Machines / Monitor / Run / MPI; c connect-all, s save, q quit" } }
                        tr { td { "BUILD" } td { "cargo workspace; cross-platform release binaries via cargo-release + git-cliff" } }
                    }
                }
            }

            section { class: "sec",
                h2 { "install" }
                pre {
                    span { class: "prompt", "$ " }
                    "cargo install --path .\n"
                    span { class: "cmt", "# or pull a prebuilt binary from releases (rolling or vX.Y.Z)" }
                }
            }

            section { class: "sec",
                h2 { "usage" }
                pre {
                    span { class: "cmt", "# global" }
                    "\nTab / ←→ / h l   switch tabs\n"
                    "c                 connect all nodes\ns                 save machines.json\nq                 quit\n\n"
                    span { class: "cmt", "# machines" }
                    "\na                 add node\nd                 delete\nD                 wipe all\n"
                    "Enter             connect\nj / k             select\n\n"
                    span { class: "cmt", "# run / mpi" }
                    "\nEnter             type command\nt                 target: all | selected\n"
                    "r / m             quick run / mpi dispatch"
                }
            }

            footer {
                a { href: "https://github.com/Michael-cmd-sys/hive", "github" }
                a { href: "https://github.com/Michael-cmd-sys/hive/releases", "releases" }
                span { "mit license" }
                span { "build 0.1.0" }
            }
        }
    }
}
