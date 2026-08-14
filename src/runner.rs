use crate::app::{Action, ConnStatus, UiEvent};
use crate::config::{ClusterConfig, MachineConfig};
use crate::jobs::{dispatch_mpi, run_on, Worker};
use crate::metrics::{MachineStats, parse_free_m, parse_loadavg, parse_mpstat, parse_nproc, parse_ps, parse_uptime};
use crate::ssh::Session;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

type Ui = UnboundedSender<UiEvent>;

async fn collect_stats(session: &mut Session) -> anyhow::Result<MachineStats> {
    let cores = parse_nproc(&session.exec("nproc").await?.stdout)?;
    let (mem_used, mem_total) = parse_free_m(&session.exec("free -m").await?.stdout)?;
    let (load1, load5, load15) = parse_loadavg(&session.exec("cat /proc/loadavg").await?.stdout)?;
    // parse_mpstat returns idle %; busy = 100 - idle
    let cpu_idle = match session.exec("mpstat 1 1 2>/dev/null || top -bn1").await {
        Ok(o) => parse_mpstat(&o.stdout).unwrap_or(100.0),
        Err(_) => 100.0,
    };
    let uptime = parse_uptime(&session.exec("cat /proc/uptime").await?.stdout)?;
    // Top processes by CPU (portable: sort in Rust, no GNU --sort needed).
    let top_procs = session
        .exec("ps -eo pid,comm,%cpu,%mem 2>/dev/null | head -n 16")
        .await
        .map(|o| {
            let mut v = parse_ps(&o.stdout);
            v.sort_by(|a, b| {
                b.cpu
                    .partial_cmp(&a.cpu)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            v.truncate(5);
            v
        })
        .unwrap_or_default();
    Ok(MachineStats {
        cores,
        cpu_percent: 100.0 - cpu_idle,
        mem_used_mib: mem_used,
        mem_total_mib: mem_total,
        load1, load5, load15,
        uptime_secs: uptime,
        top_procs,
    })
}

async fn spawn_machine(mc: MachineConfig, ui: Ui, poll_ms: u64, password: Option<String>) {
    ui.send(UiEvent::Conn { name: mc.name.clone(), status: ConnStatus::Connecting }).ok();
    let mut session = match Session::connect(&mc, password.as_deref()).await {
        Ok(s) => s,
        Err(e) => {
            ui.send(UiEvent::Conn { name: mc.name.clone(), status: ConnStatus::Error(e.to_string()) }).ok();
            return;
        }
    };
    ui.send(UiEvent::Conn { name: mc.name.clone(), status: ConnStatus::Connected }).ok();
    let mut consecutive_errors: u32 = 0;
    loop {
        match collect_stats(&mut session).await {
            Ok(stats) => {
                consecutive_errors = 0;
                ui.send(UiEvent::Stats { name: mc.name.clone(), stats }).ok();
            }
            Err(e) => {
                ui.send(UiEvent::Log(format!("{} stats err: {e}", mc.name))).ok();
                consecutive_errors += 1;
                if consecutive_errors >= 3 {
                    ui.send(UiEvent::Conn {
                        name: mc.name.clone(),
                        status: ConnStatus::Error("connection lost / metrics failing".into()),
                    }).ok();
                    break;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;
    }
}

pub async fn run_dispatcher(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Action>,
    ui: Ui,
    cfg: Arc<Mutex<ClusterConfig>>,
    secrets: Arc<Mutex<HashMap<String, String>>>,
) {
    let mut handles: HashMap<String, tokio::task::JoinHandle<()>> = HashMap::new();
    while let Some(action) = rx.recv().await {
        match action {
            Action::Quit => break,
            Action::SeedSecret { name, password } => {
                secrets.lock().unwrap().insert(name, password);
            }
            Action::ConnectAll | Action::Connect(_) => {
                let poll = cfg.lock().unwrap().poll_interval_ms;
                let machines: Vec<MachineConfig> = cfg.lock().unwrap().machines.clone();
                for mc in machines {
                    let need = match &action {
                        Action::Connect(n) => &mc.name == n,
                        _ => true,
                    };
                    if need {
                        let ui2 = ui.clone();
                        let secrets2 = secrets.clone();
                        let name = mc.name.clone();
                        if let Some(old) = handles.get(&name) {
                            old.abort();
                        }
                        let handle_name = name.clone();
                        let h = tokio::spawn(async move {
                            let pw = secrets2.lock().unwrap().get(&name).cloned();
                            spawn_machine(mc, ui2, poll, pw).await
                        });
                        handles.insert(handle_name, h);
                    }
                }
            }
            Action::Disconnect(name) => {
                if let Some(h) = handles.remove(&name) {
                    h.abort();
                }
                ui.send(UiEvent::Conn { name: name.clone(), status: ConnStatus::Disconnected }).ok();
            }
            Action::RemoveMachine(name) => {
                if let Some(h) = handles.remove(&name) {
                    h.abort();
                }
                secrets.lock().unwrap().remove(&name);
                {
                    let mut guard = cfg.lock().unwrap();
                    guard.machines.retain(|m| m.name != name);
                    let path = std::env::var("HIVE_CONFIG").unwrap_or_else(|_| "cluster.yaml".into());
                    if let Err(e) = guard.save(std::path::Path::new(&path)) {
                        ui.send(UiEvent::Log(format!("save failed: {e}"))).ok();
                    } else {
                        ui.send(UiEvent::Log(format!("{name} removed and saved"))).ok();
                    }
                }
                ui.send(UiEvent::Removed { name }).ok();
            }
            Action::ClearAll => {
                for (_, h) in handles.drain() {
                    h.abort();
                }
                secrets.lock().unwrap().clear();
                {
                    let mut guard = cfg.lock().unwrap();
                    guard.machines.clear();
                    let path = std::env::var("HIVE_CONFIG").unwrap_or_else(|_| "cluster.yaml".into());
                    if let Err(e) = guard.save(std::path::Path::new(&path)) {
                        ui.send(UiEvent::Log(format!("save failed: {e}"))).ok();
                    } else {
                        ui.send(UiEvent::Log("all machines wiped".into())).ok();
                    }
                }
                ui.send(UiEvent::Cleared).ok();
            }
            Action::SaveConfig => {
                let path = std::env::var("HIVE_CONFIG").unwrap_or_else(|_| "cluster.yaml".into());
                let guard = cfg.lock().unwrap();
                if let Err(e) = guard.save(std::path::Path::new(&path)) {
                    ui.send(UiEvent::Log(format!("save failed: {e}"))).ok();
                } else {
                    ui.send(UiEvent::Log("config saved".into())).ok();
                }
            }
            Action::Run { targets, cmd } => {
                let machines: Vec<MachineConfig> = cfg.lock().unwrap().machines.clone();
                for name in targets {
                    if let Some(mc) = machines.iter().find(|m| m.name == name) {
                        let pw = secrets.lock().unwrap().get(&name).cloned();
                        match Session::connect(mc, pw.as_deref()).await {
                            Ok(mut s) => match run_on(&mut s, &cmd).await {
                                Ok(out) => ui.send(UiEvent::RunOutput { name, out }).ok(),
                                Err(e) => ui.send(UiEvent::Log(format!("{name}: {e}"))).ok(),
                            },
                            Err(e) => ui.send(UiEvent::Log(format!("{name} connect: {e}"))).ok(),
                        };
                    }
                }
            }
            Action::LaunchMpi { head, workers, binary, args } => {
                let machines: Vec<MachineConfig> = cfg.lock().unwrap().machines.clone();
                let head_cfg = match machines.iter().find(|m| m.name == head) {
                    Some(c) => c.clone(),
                    None => { ui.send(UiEvent::Log(format!("head {head} not found"))).ok(); continue; }
                };
                let pw = secrets.lock().unwrap().get(&head).cloned();
                match Session::connect(&head_cfg, pw.as_deref()).await {
                    Ok(mut s) => {
                        let ws: Vec<Worker> = workers.iter().filter_map(|n| {
                            machines.iter().find(|m| &m.name == n).map(|m| Worker {
                                host: m.host.clone(),
                                slots: 4,
                            })
                        }).collect();
                        let cfg_snapshot = cfg.lock().unwrap().clone();
                        match dispatch_mpi(&mut s, &cfg_snapshot, &ws, &binary, &args).await {
                            Ok(out) => ui.send(UiEvent::MpiOutput(out)).ok(),
                            Err(e) => ui.send(UiEvent::MpiOutput(format!("ERR: {e}"))).ok(),
                        };
                    }
                    Err(e) => { ui.send(UiEvent::MpiOutput(format!("head connect: {e}"))).ok(); }
                };
            }
        }
    }
}
