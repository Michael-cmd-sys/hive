use dioxus::prelude::*;

// Static telemetry snapshot — per-core CPU load per node (0-100%).
// Used to demonstrate the live Monitor view on the landing page.
const NODES: &[(&str, &[u8], u8, &str)] = &[
    ("node-00", &[38, 42, 51, 40, 30, 45, 39, 44], 60, "OK"),
    ("node-01", &[70, 65, 80, 60, 72, 68, 55, 77], 72, "OK"),
    ("node-02", &[92, 88, 95, 85, 90, 99, 84, 91], 91, "HOT"),
    ("node-03", &[48, 52, 47, 55, 49, 51, 53, 50], 40, "OK"),
    ("node-04", &[5, 8, 3, 12, 6, 9, 4, 10], 12, "IDLE"),
    ("node-05", &[73, 68, 80, 71, 75, 69, 77, 72], 80, "OK"),
    ("node-06", &[95, 90, 99, 93, 96, 88, 97, 94], 96, "HOT"),
    ("node-07", &[28, 33, 25, 30, 35, 27, 31, 29], 35, "OK"),
];

const BENEFITS: &[(&str, &str)] = &[
    (
        "Per-core visibility",
        "Node averages hide the truth. hive shows load on every individual core, so you can see exactly which ones are saturated — and which are sitting idle.",
    ),
    (
        "Fleet-wide commands",
        "Type a command once and run it on every node. Output streams back into a single view — no SSH loops, no copy-paste, no scripts to maintain.",
    ),
    (
        "Zero agents",
        "Nothing to deploy on your nodes. hive uses the SSH and shell already there. If you can ssh in, hive can manage it.",
    ),
    (
        "Secure by default",
        "Connection passwords live only in memory for the life of the session and are never written to disk.",
    ),
];

const STEPS: &[(&str, &str)] = &[
    (
        "01 / Describe your cluster",
        "Add machines interactively — name, host, SSH user, and a password or key — or drop a cluster.yaml beside the binary. No hand-editing of config required.",
    ),
    (
        "02 / Connect and watch",
        "Press c to connect everything. Per-node CPU, RAM, and load average update live, every two seconds.",
    ),
    (
        "03 / Command or dispatch",
        "Run a shell command across the fleet, or launch an MPI job with an auto-generated hostfile and per-node process counts.",
    ),
];

const SPECS: &[(&str, &str)] = &[
    (
        "Transport",
        "Authenticated SSH via libssh2. Passwords are captured at runtime, held in memory, and never persisted to disk.",
    ),
    (
        "Dispatch",
        "Ad-hoc shell across all or selected nodes; MPI jobs launched with mpirun using an auto-generated hostfile.",
    ),
    (
        "Telemetry",
        "Per-node CPU — broken down per core — plus RAM and load average, sampled every 2 seconds.",
    ),
    (
        "Interface",
        "A fast terminal UI (ratatui) with tabbed Machines, Monitor, Run, and MPI views.",
    ),
    (
        "Packaging",
        "Written in Rust; cross-compiled release binaries for Linux, macOS, and Windows.",
    ),
];

fn main() {
    launch(app);
}

fn app() -> Element {
    rsx! {
        document::Stylesheet { href: asset!("/assets/tailwind.css") }
        div { class: "max-w-[960px] mx-auto px-5 py-12 relative",

            // ── Hero ───────────────────────────────────────────────
            section { class: "relative border border-line bg-panel px-6 py-10 sm:px-10",
                span { class: "absolute -top-3 left-5 bg-bg px-2 text-xs tracking-[0.2em] text-fg-dim",
                    "SYSTEM // hive"
                }
                h1 { class: "glow text-fg font-bold text-[clamp(2rem,6vw,3.4rem)] leading-tight tracking-tight m-0",
                    "Command your whole cluster from one terminal."
                }
                p { class: "text-fg-dim mt-4 max-w-[60ch] text-[15px] leading-relaxed",
                    "hive is keyboard-driven SSH orchestration for labs and HPC clusters. See live per-core load on every node, run one command across the fleet, and launch MPI jobs — without installing anything on the machines you manage."
                }
                div { class: "mt-7 flex flex-wrap gap-3",
                    a { class: "inline-block border border-amber text-amber px-5 py-2 text-sm font-bold tracking-wide hover:bg-amber/10 transition-colors",
                        href: "#install", "Install hive" }
                    a { class: "inline-block border border-line text-fg px-5 py-2 text-sm tracking-wide hover:bg-fg/5 transition-colors",
                        href: "https://github.com/Michael-cmd-sys/hive", "View source on GitHub" }
                }
            }

            // ── Why ────────────────────────────────────────────────
            section { class: "mt-16",
                h2 { class: "text-sm tracking-[3px] uppercase text-fg-dim border-b border-line pb-2 mb-6",
                    "Why operators choose hive" }
                div { class: "grid gap-4 sm:grid-cols-2",
                    for (title, body) in BENEFITS {
                        div { class: "border border-line bg-[#06100d] p-5",
                            h3 { class: "text-fg font-bold text-base m-0", "{title}" }
                            p { class: "text-fg-dim mt-2 text-sm leading-relaxed", "{body}" }
                        }
                    }
                }
            }

            // ── Live demo ──────────────────────────────────────────
            section { class: "mt-16",
                h2 { class: "text-sm tracking-[3px] uppercase text-fg-dim border-b border-line pb-2 mb-4",
                    "Live cluster telemetry" }
                p { class: "text-fg-dim text-sm mb-5 max-w-[65ch] leading-relaxed",
                    "A snapshot of the Monitor tab: every core of every node, color-coded by load. Green is healthy, amber is busy, red is hot — so a single saturated core is impossible to miss."
                }
                div { class: "grid gap-3 grid-cols-[repeat(auto-fill,minmax(160px,1fr))]",
                    for (id, cores, _ram, state) in NODES {
                        {
                            let avg = cores.iter().map(|c| *c as u32).sum::<u32>() / cores.len() as u32;
                            rsx! {
                                div { class: "border border-line bg-[#06100d] p-3 text-xs",
                                    div { class: "flex justify-between items-baseline",
                                        span { class: "text-fg", "{id}" }
                                        span { class: "text-fg-dim", "{cores.len()}c" }
                                    }
                                    div { class: "flex items-end gap-[2px] h-12 mt-2",
                                        for load in cores {
                                            {
                                                let bar_cls = if *load >= 85 { "bg-red" } else if *load >= 70 { "bg-amber" } else { "bg-fg" };
                                                rsx! {
                                                    div { class: "flex-1 h-full bg-[#0a1b16] border border-line flex items-end",
                                                        div { class: "w-full {bar_cls}", style: "height: {load}%" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    div { class: "text-fg-dim mt-2", "avg {avg}% cpu · {state}" }
                                }
                            }
                        }
                    }
                }
            }

            // ── How ────────────────────────────────────────────────
            section { class: "mt-16",
                h2 { class: "text-sm tracking-[3px] uppercase text-fg-dim border-b border-line pb-2 mb-6",
                    "Up and running in three steps" }
                div { class: "space-y-4",
                    for (title, body) in STEPS {
                        div { class: "border-l-2 border-amber pl-4",
                            h3 { class: "text-fg font-bold text-base m-0", "{title}" }
                            p { class: "text-fg-dim mt-1 text-sm leading-relaxed max-w-[70ch]", "{body}" }
                        }
                    }
                }
            }

            // ── Install ────────────────────────────────────────────
            section { class: "mt-16", id: "install",
                h2 { class: "text-sm tracking-[3px] uppercase text-fg-dim border-b border-line pb-2 mb-5",
                    "Get hive" }
                p { class: "text-fg-dim text-sm leading-relaxed max-w-[70ch]",
                    "hive ships as a single, self-contained binary. The quickest way to install it is with Cargo, Rust's package manager. First grab the source, move into the project folder, then build and install:"
                }
                pre { class: "bg-[#020503] border border-line p-4 mt-3 overflow-x-auto text-sm text-fg m-0",
                    span { class: "text-amber", "$ " }
                    "git clone https://github.com/Michael-cmd-sys/hive\n"
                    span { class: "text-amber", "$ " }
                    "cd hive\n"
                    span { class: "text-amber", "$ " }
                    "cargo install --path ."
                }
                p { class: "text-fg-dim text-sm leading-relaxed max-w-[70ch] mt-4",
                    "The "
                    code { class: "text-fg border border-line px-1", "git clone" }
                    " step downloads the project; "
                    code { class: "text-fg border border-line px-1", "cd hive" }
                    " moves you into it; and "
                    code { class: "text-fg border border-line px-1", "cargo install --path ." }
                    " compiles hive from that local source and adds the "
                    code { class: "text-fg border border-line px-1", "hive" }
                    " command to your terminal's PATH. You will need Git, Rust (Cargo), and SSH access to the machines you want to manage."
                }
                p { class: "text-fg-dim text-sm leading-relaxed max-w-[70ch] mt-4",
                    "No Rust toolchain? Download a prebuilt binary for Linux, macOS, or Windows from the "
                    a { class: "text-amber border-b border-dashed border-amber hover:bg-amber/10",
                        href: "https://github.com/Michael-cmd-sys/hive/releases", "Releases page" }
                    " — nothing to compile."
                }
                p { class: "text-fg-dim text-sm leading-relaxed max-w-[70ch] mt-4",
                    "Then just run "
                    code { class: "text-fg border border-line px-1", "hive" }
                    ". Press "
                    span { class: "text-fg border border-line px-1", "c" }
                    " to connect your cluster and you are monitoring in seconds."
                }
            }

            // ── Under the hood ─────────────────────────────────────
            section { class: "mt-16",
                h2 { class: "text-sm tracking-[3px] uppercase text-fg-dim border-b border-line pb-2 mb-5",
                    "Under the hood" }
                table { class: "w-full border-collapse text-sm",
                    tbody {
                        for (label, body) in SPECS {
                            tr { td { class: "border border-line p-3 align-top text-amber whitespace-nowrap w-[1%]", "{label}" }
                                td { class: "border border-line p-3 align-top text-fg-dim", "{body}" } }
                        }
                    }
                }
            }

            footer { class: "mt-14 border-t border-line pt-5 text-fg-dim text-xs flex flex-wrap gap-5",
                a { class: "text-amber border-b border-dashed border-amber hover:bg-amber/10",
                    href: "https://github.com/Michael-cmd-sys/hive", "github" }
                a { class: "text-amber border-b border-dashed border-amber hover:bg-amber/10",
                    href: "https://github.com/Michael-cmd-sys/hive/releases", "releases" }
                span { "MIT license" }
                span { "build 0.1.0" }
            }
        }
    }
}
