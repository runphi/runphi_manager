//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//*********************************************

use clap::{CommandFactory, Parser};
use serde_json;
use std::error::Error;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::process::exit;

use liboci_cli::{GlobalOpts, StandardCmd};

// High-level commandline option definition
// This takes global options as well as individual commands as specified in [OCI runtime-spec](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md)
// Also check [runc commandline documentation](https://github.com/opencontainers/runc/blob/master/man/runc.8.md) for more explanation
#[derive(Parser, Debug)]
#[clap(version = "0.5.0", author = env!("CARGO_PKG_AUTHORS"))]
struct Opts {
    #[clap(flatten)]
    global: GlobalOpts,

    #[clap(subcommand)]
    subcmd: SubCommand,
}

// Subcommands accepted by Youki, confirming with [OCI runtime-spec](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md)
// Also for a short information, check [runc commandline documentation](https://github.com/opencontainers/runc/blob/master/man/runc.8.md)
#[derive(Parser, Debug)]
enum SubCommand {
    // Standard and common commands handled by the liboci_cli crate
    #[clap(flatten)]
    Standard(Box<liboci_cli::StandardCmd>),
    #[clap(flatten)]
    Common(Box<liboci_cli::CommonCmd>),
}

mod frontend {
    pub mod commands;
}
mod forwarding;

//TODO: convert strings to Path and PathBuf
const WORKPATH: &str = "/usr/share/runPHI";
const RUNDIR: &str = "/run/runPHI";

fn main() -> Result<(), Box<dyn Error>> {
    // Check if there is at least one runtime available, exit otherwise
    
    // TODO: "configuration" file is needed to check if a specific backend is enabled
    // currently, only Jailhouse is supported. Replace "configuration" file with an efficient mechanism
    // that is backend dependent (e.g., Jailhouse enabled should be checked with jailhouse.ko
    // loaded)

    let _ = match fs::read_to_string(format!("{}/configuration", WORKPATH)) {
        Ok(content) => {
            if content.trim().is_empty() {
                writeln!(io::stderr(), "No runPHI runtime available, forwarding")?;
                forwarding::call_runc();
                exit(10);
            } else {
                Some(content)
            }
        }
        Err(err) => {
            eprintln!("Error reading file: {}", err);
            forwarding::call_runc();
            exit(9);
        }
    };

    let containerid;
    let mut config: serde_json::Value = serde_json::Value::Null;
    let mut logfile = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/usr/share/runPHI/log.txt")?;

    let opts = Opts::parse(); //RIMUOVERE. questo parsa le opzioni da riga di comando e le mette in Opts
    let _app = Opts::command(); //restituisce un oggetto command che rappresenta la struttura della cli?????

    let _ = match opts.subcmd { //se opts.subcmd è un subcommand standard
        SubCommand::Standard(cmd) => match *cmd { //allora matcha cmd,
            // We here distinguish the behaviour by command defined as OCI spec
            // Common to all commands, take the first 24 chars to get the containerID
            // RunPHI restricts the ID to 24 chars because hypervisors like Jailhouse may fail with
            // a partition of a longer name
            // After collecting the ID, we have to check if we need to forward to runc because container
            // does not belong to RunPHI management cycle.
            //TODO: fix common part handling
            StandardCmd::Create(create) => { //se è create automaticamente in create ci vengono messe tutte le opzioni da linea di comando
                containerid = create.container_id.chars().take(24).collect::<String>();
                let _ = writeln!(logfile, "Creating with id {}", &containerid); //DEBUG
                let _ = writeln!(logfile, "Parse json"); //DEBUG
                let config_json = fs::read_to_string(format!(
                    "{}/config.json",
                    &create.bundle.to_string_lossy().into_owned()
                ))?;
                config = serde_json::from_str(&config_json)?;
                forwarding::runc_forward_ifnecessary(&config, &containerid);
                let _ = writeln!(
                    logfile,
                    "Not forwarded containerid creation {}",
                    &containerid
                ); //DEBUG
                   // If we are here, there was no forwarding to runc, hence we start runphi management
                let crundir = format!("{}/{}", RUNDIR, containerid);
                //TODO: fix?, this should not exist
                fs::remove_dir_all(&crundir).ok();
                //Create container directory to store runphi-related information
                fs::create_dir_all(&crundir)?;

                let _ = frontend::commands::create(&containerid, create, &crundir, config);
            }
            StandardCmd::Start(start) => {
                containerid = start.container_id.chars().take(24).collect::<String>();
                let _ = writeln!(logfile, "Starting with id {}", &containerid); //DEBUG
                forwarding::runc_forward_ifnecessary(&config, &containerid);
                let _ = writeln!(logfile, "Starting with id not forwarded {}", &containerid); //DEBUG
                let crundir = format!("{}/{}", RUNDIR, containerid);
                let _ = frontend::commands::start(&containerid, &crundir);
            }
            StandardCmd::Kill(kill) => {
                containerid = kill.container_id.chars().take(24).collect::<String>();
                let _ = writeln!(logfile, "Killing with id {}", &containerid); //DEBUG
                forwarding::runc_forward_ifnecessary(&config, &containerid);
                let crundir = format!("{}/{}", RUNDIR, containerid);
                let _ = frontend::commands::kill(&containerid, &crundir);
            }

            StandardCmd::Delete(delete) => {
                containerid = delete.container_id.chars().take(24).collect::<String>();
                let _ = writeln!(logfile, "Deleting with id {}", &containerid); //DEBUG
                forwarding::runc_forward_ifnecessary_delete(&config, &containerid);
                let crundir = format!("{}/{}", RUNDIR, containerid);
                let _ = frontend::commands::delete(&containerid, &crundir);
            }

            StandardCmd::State(state) => {
                containerid = state.container_id.chars().take(24).collect::<String>();
                let _ = writeln!(logfile, "State with id {}", &containerid); //DEBUG
                forwarding::runc_forward_ifnecessary(&config, &containerid);
                let crundir = format!("{}/{}", RUNDIR, containerid);
                let _ = frontend::commands::state(&containerid, &crundir);
            }
        },
        SubCommand::Common(_) => {} /* Unimplemented yet
                                    match *cmd {
                                        CommonCmd::Checkpoint(checkpoint) => {
                                            commands::checkpoint::checkpoint(checkpoint, root_path)
                                        }
                                        CommonCmd::Events(events) => commands::events::events(events, root_path),
                                        CommonCmd::Exec(exec) => match commands::exec::exec(exec, root_path) {
                                            Ok(exit_code) => std::process::exit(exit_code),
                                            Err(e) => {
                                                //tracing::error!("error in executing command: {:?}", e);
                                                eprintln!("exec failed : {e}");
                                                std::process::exit(-1);
                                            }
                                        },
                                        CommonCmd::Features(features) => commands::features::features(features),
                                        CommonCmd::List(list) => commands::list::list(list, root_path),
                                        CommonCmd::Pause(pause) => commands::pause::pause(pause, root_path),
                                        CommonCmd::Ps(ps) => commands::ps::ps(ps, root_path),
                                        CommonCmd::Resume(resume) => commands::resume::resume(resume, root_path),
                                        CommonCmd::Run(run) => match commands::run::run(run, root_path, systemd_cgroup) {
                                            Ok(exit_code) => std::process::exit(exit_code),
                                            Err(e) => {
                                                //tracing::error!("error in executing command: {:?}", e);
                                                eprintln!("run failed : {e}");
                                                std::process::exit(-1);
                                            }
                                        },
                                        CommonCmd::Spec(spec) => commands::spec_json::spec(spec),
                                        CommonCmd::Update(update) => commands::update::update(update, root_path),
                                    },

                                    SubCommand::Info(info) => commands::info::info(info),
                                    SubCommand::Completion(completion) => {
                                        commands::completion::completion(completion, &mut app)
                                    } */
    };

    return Ok(());
}
