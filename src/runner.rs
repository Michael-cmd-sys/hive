use crate::app::{Action, ConnStatus, UiEvent};
use crate::config::{ClusterConfig, MachineConfig};
use crate::jobs::{dispatch_mpi, run_on, Worker};
use crate::metrics::{MachineStats, parse_free_m, parse_loadavg, parse_mpstat, parse_nproc, parse_uptime};
use crate::ssh::Session;
use std::sync::Arc;
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
    Ok(MachineStats {
        cores,
        cpu_percent: 100.0 - cpu_idle,
        mem_used_mib: mem_used,
        mem_total_mib: mem_total,
        load1, load5, load15,
        uptime_secs: uptime,
    })
}

async fn spawn_machine(mc: MachineConfig, ui: Ui, poll_ms: u64) {
    ui.send(UiEvent::Conn { name: mc.name.clone(), status: ConnStatus::Connecting }).ok();
    let mut session = match Session::connect(&mc).await {
        Ok(s) => s,
        Err(e) => {
            ui.send(UiEvent::Conn { name: mc.name.clone(), status: ConnStatus::Error(e.to_string()) }).ok();
            return;
        }
    };
    ui.send(UiEvent::Conn { name: mc.name.clone(), status: ConnStatus::Connected }).ok();
    loop {
        match collect_stats(&mut session).await {
            Ok(stats) => { ui.send(UiEvent::Stats { name: mc.name.clone(), stats }).ok(); }
            Err(e) => { ui.send(UiEvent::Log(format!("{} stats err: {e}", mc.name))).ok(); }
        }
        tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;
    }
}

pub async fn run_dispatcher(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Action>,
    ui: Ui,
    cfg: Arc<ClusterConfig>,
) {
    while let Some(action) = rx.recv().await {
        match action {
            Action::Quit => break,
            Action::ConnectAll | Action::Connect(_) => {
                let poll = cfg.poll_interval_ms;
                for mc in &cfg.machines {
                    let need = match &action {
                        Action::Connect(n) => &mc.name == n,
                        _ => true,
                    };
                    if need {
                        let ui2 = ui.clone();
                        let mc2 = mc.clone();
                        tokio::spawn(async move { spawn_machine(mc2, ui2, poll).await });
                    }
                }
            }
            Action::Disconnect(_) => { /* poller keeps running; acceptable for this iteration */ }
            Action::SaveConfig(c) => {
                let path = std::env::var("HIVE_CONFIG").unwrap_or_else(|_| "cluster.yaml".into());
                if let Err(e) = c.save(std::path::Path::new(&path)) {
                    ui.send(UiEvent::Log(format!("save failed: {e}"))).ok();
                } else {
                    ui.send(UiEvent::Log("config saved".into())).ok();
                }
            }
            Action::Run { targets, cmd } => {
                for name in targets {
                    if let Some(mc) = cfg.machines.iter().find(|m| m.name == name) {
                        match Session::connect(mc).await {
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
                let head_cfg = match cfg.machines.iter().find(|m| m.name == head) {
                    Some(c) => c.clone(),
                    None => { ui.send(UiEvent::Log(format!("head {head} not found"))).ok(); continue; }
                };
                match Session::connect(&head_cfg).await {
                    Ok(mut s) => {
                        let ws: Vec<Worker> = workers.iter().filter_map(|n| {
                            cfg.machines.iter().find(|m| &m.name == n).map(|m| Worker {
                                host: m.host.clone(),
                                slots: 4,
                            })
                        }).collect();
                        match dispatch_mpi(&mut s, &cfg, &ws, &binary, &args).await {
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
