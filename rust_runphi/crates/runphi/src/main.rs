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
use std::path::{Path, PathBuf};

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
// reports which hypervisor it was built for. The leading version stays
// as the first version token so existing scripts that awk field 2
// (get_current_container_runtime.sh) keep working.
#[cfg(feature = "jailhouse")]
const VERSION_STR: &str = "0.5.8 (backend: jailhouse)";
#[cfg(feature = "xen")]
const VERSION_STR: &str = "0.5.8 (backend: xen)";

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

use f2b::paths;
use forwarding::ForwardDecision;

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

// Container IDs are truncated to 24 chars because hypervisors like
// Jailhouse may fail with a partition name longer than that.
fn truncate_id(raw: &str) -> String {
    raw.chars().take(24).collect()
}

// Boilerplate shared by Start / Kill / Delete / State: truncate the
// container ID, decide whether to forward to runc (and exit if so),
// log the action, and build the per-container state directory path.
fn dispatch_existing(raw_id: &str, verb: &str) -> (String, PathBuf) {
    let containerid = truncate_id(raw_id);
    forward_or_continue(forwarding::decide_existing(&containerid), &containerid);
    logging::log_message(
        logging::Level::Info,
        format!("{} with id {}", verb, &containerid).as_str(),
    );
    let crundir = Path::new(paths::STATE_DIR).join(&containerid);
    (containerid, crundir)
}

fn main() -> Result<(), Box<dyn Error>> {

    // Initialize the logging and timer modules
    logging::init_logger(Some(std::path::PathBuf::from(logging::LOG_PATH)));//opts.global.log);
    // The timer is only used for optional boot-time instrumentation and is
    // backed by a platform device that may be absent (e.g. the Xen backend's
    // /dev/arm_timer is ARM-only and does not exist on an x86 Xen Dom0). A
    // missing timer must never stop runphi from managing containers, so treat
    // install failure as non-fatal: log it and continue. With no source
    // installed, logging::timer::capture() simply returns 0.
    if let Err(e) = backend::timer::install() {
        logging::log_message(
            logging::Level::Warn,
            format!(
                "timer source unavailable, continuing without boot-time instrumentation: {}",
                e
            )
            .as_str(),
        );
    }

    //timer::log_phase("runPHI main started")?; //To log the time of runphi start

    let opts = Opts::parse();
    //let _app = Opts::command();

    match opts.subcmd {
        SubCommand::Standard(cmd) => match *cmd {
            // For each OCI command we (a) truncate the container ID to 24
            // chars (Jailhouse limit), (b) decide whether to forward the
            // call to vanilla runc, (c) log the action, and (d) build the
            // per-container state directory path. dispatch_existing bundles
            // (a-d) for everything except `create`, which has its own
            // bootstrap flow because the state dir doesn't exist yet.
            StandardCmd::Create(create) => {
                let containerid = truncate_id(&create.container_id);
                logging::log_message(logging::Level::Debug, "Parse json");
                let config_json = fs::read_to_string(create.bundle.join("config.json"))?;
                let config: serde_json::Value = serde_json::from_str(&config_json)?;

                forward_or_continue(
                    forwarding::decide_create(&config, &create.bundle),
                    &containerid,
                );

                // If we are here, there was no forwarding to runc, hence we start runphi management
                logging::log_message(
                    logging::Level::Info,
                    format!("Creating with id {}", &containerid).as_str(),
                );
                let crundir: PathBuf = Path::new(paths::STATE_DIR).join(&containerid);
                //TODO: fix?, this should not exist
                fs::remove_dir_all(&crundir).ok();
                //Create container directory to store runphi-related information
                fs::create_dir_all(&crundir)?;

                frontend::commands::create(&containerid, create, &crundir, config)?;
            }
            StandardCmd::Start(start) => {
                let (containerid, crundir) = dispatch_existing(&start.container_id, "Starting");
                frontend::commands::start(&containerid, &crundir)?;
            }
            StandardCmd::Kill(kill) => {
                let (containerid, crundir) = dispatch_existing(&kill.container_id, "Killing");
                frontend::commands::kill(&containerid, &crundir)?;
            }
            StandardCmd::Delete(delete) => {
                let (containerid, crundir) = dispatch_existing(&delete.container_id, "Deleting");
                frontend::commands::delete(&containerid, &crundir)?;
            }
            StandardCmd::State(state) => {
                let (containerid, crundir) = dispatch_existing(&state.container_id, "State");
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
