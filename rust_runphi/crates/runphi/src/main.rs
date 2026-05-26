//*********************************************
// Authors: 
// Marco Barletta (marco.barletta@unina.it)
// Francesco Tafuri (fran.tafuri@studenti.unina.it)
// Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

//use clap::{CommandFactory, Parser};
use clap::Parser;
use std::error::Error;
use std::fs;

use liboci_cli::{GlobalOpts, StandardCmd};

// Backend selection. Exactly one of the `jailhouse` / `xen` Cargo
// features must be enabled at build time; the selected backend crate
// is re-exported under the unified name `backend` for the rest of
// runphi (and its submodules) to use.
#[cfg(feature = "jailhouse")]
pub use backend_jailhouse as backend;
#[cfg(feature = "xen")]
pub use backend_xen as backend;

#[cfg(not(any(feature = "jailhouse", feature = "xen")))]
compile_error!("Select a backend: --features jailhouse or --features xen");
#[cfg(all(feature = "jailhouse", feature = "xen"))]
compile_error!("Backends jailhouse and xen are mutually exclusive");

// Version string surfaces the active backend so `runphi --version`
// reports which hypervisor it was built for. The leading "0.5.7" stays
// as the first version token so existing scripts that awk field 2
// (get_current_container_runtime.sh) keep working.
#[cfg(feature = "jailhouse")]
const VERSION_STR: &str = "0.5.7 (backend: jailhouse)";
#[cfg(feature = "xen")]
const VERSION_STR: &str = "0.5.7 (backend: xen)";

// High-level commandline option definition
// This takes global options as well as individual commands as specified in [OCI runtime-spec](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md)
// Also check [runc commandline documentation](https://github.com/opencontainers/runc/blob/master/man/runc.8.md) for more explanation
#[derive(Parser, Debug)]
#[clap(version = VERSION_STR, author = env!("CARGO_PKG_AUTHORS"))]
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

use forwarding::ForwardDecision;

//TODO: convert strings to Path and PathBuf
//const WORKPATH: &str = "/usr/share/runPHI";
const RUNDIR: &str = forwarding::RUNPHI_STATE_DIR;

// Forward to runc and exit with its status, or continue with runPHI based on `decision`.
fn forward_or_continue(decision: ForwardDecision, containerid: &str) {
    if let ForwardDecision::ForwardToRunc = decision {
        logging::log_message(
            logging::Level::Info,
            format!("Forwarding to runc id {}", containerid).as_str(),
        );
        std::process::exit(forwarding::call_runc());
    }
}

fn main() -> Result<(), Box<dyn Error>> {

    // Initialize the logging and timer modules
    logging::init_logger(Some(std::path::PathBuf::from(std::path::Path::new("/usr/share/runPHI/log.txt"))));//opts.global.log);
    backend::timer::install()?;

    //timer::log_phase("runPHI main started")?; //To log the time of runphi start

    let containerid;
    let opts = Opts::parse();
    //let _app = Opts::command();

    match opts.subcmd {
        SubCommand::Standard(cmd) => match *cmd {
            // We here distinguish the behaviour by command defined as OCI spec
            // Common to all commands, take the first 24 chars to get the containerID
            // RunPHI restricts the ID to 24 chars because hypervisors like Jailhouse may fail with
            // a partition of a longer name
            // After collecting the ID, we have to check if we need to forward to runc because container
            // does not belong to RunPHI management cycle.
            //TODO: fix common part handling
            StandardCmd::Create(create) => {
                containerid = create.container_id.chars().take(24).collect::<String>();
                logging::log_message(logging::Level::Debug,  "Parse json");
                let config_json = fs::read_to_string(format!(
                    "{}/config.json",
                    &create.bundle.to_string_lossy().into_owned()))?;
                let config: serde_json::Value = serde_json::from_str(&config_json)?;

                forward_or_continue(
                    forwarding::decide_create(&config, &create.bundle),
                    &containerid,
                );

                // If we are here, there was no forwarding to runc, hence we start runphi management
                logging::log_message(logging::Level::Info,  format!("Creating with id {}", &containerid).as_str());
                let crundir = format!("{}/{}", RUNDIR, containerid);
                //TODO: fix?, this should not exist
                fs::remove_dir_all(&crundir).ok();
                //Create container directory to store runphi-related information
                fs::create_dir_all(&crundir)?;

                frontend::commands::create(&containerid, create, &crundir, config)?;
            }
            StandardCmd::Start(start) => {
                containerid = start.container_id.chars().take(24).collect::<String>();
                forward_or_continue(forwarding::decide_existing(&containerid), &containerid);
                logging::log_message(logging::Level::Info,  format!("Starting with id {}", &containerid).as_str());
                let crundir = format!("{}/{}", RUNDIR, containerid);
                frontend::commands::start(&containerid, &crundir)?;
            }
            StandardCmd::Kill(kill) => {
                containerid = kill.container_id.chars().take(24).collect::<String>();
                forward_or_continue(forwarding::decide_existing(&containerid), &containerid);
                logging::log_message(logging::Level::Info,  format!("Killing with id {}", &containerid).as_str());
                let crundir = format!("{}/{}", RUNDIR, containerid);
                frontend::commands::kill(&containerid, &crundir)?;
            }

            StandardCmd::Delete(delete) => {
                containerid = delete.container_id.chars().take(24).collect::<String>();
                forward_or_continue(forwarding::decide_existing(&containerid), &containerid);
                logging::log_message(logging::Level::Info,  format!("Deleting with id {}", &containerid).as_str());
                let crundir = format!("{}/{}", RUNDIR, containerid);
                frontend::commands::delete(&containerid, &crundir)?;
            }

            StandardCmd::State(state) => {
                containerid = state.container_id.chars().take(24).collect::<String>();
                forward_or_continue(forwarding::decide_existing(&containerid), &containerid);
                logging::log_message(logging::Level::Info,  format!("State with id {}", &containerid).as_str());
                let crundir = format!("{}/{}", RUNDIR, containerid);
                frontend::commands::state(&containerid, &crundir)?;
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

    //log_timestamp_with_memory_mmap("end main", log_file, mem_address, mem_size).unwrap();
    Ok(())
}
