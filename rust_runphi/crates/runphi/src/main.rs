//*********************************************
// Authors: 
// Marco Barletta (marco.barletta@unina.it)
// Francesco Tafuri (fran.tafuri@studenti.unina.it)
//*********************************************

//use clap::{CommandFactory, Parser};
use clap::Parser;
use serde_json;
use std::error::Error;
use std::fs;

//LIBRARIES FOR log_timestamp_with_memory_mmap function
//use std::fs::OpenOptions;
//use std::io::Write;
//use std::os::unix::io::AsRawFd;
//use std::time::{SystemTime, UNIX_EPOCH};
//use nix::libc::{mmap, munmap, MAP_SHARED, PROT_READ};

//use std::ptr;
//use std::process::exit;

use liboci_cli::{GlobalOpts, StandardCmd};
use logging;

// High-level commandline option definition
// This takes global options as well as individual commands as specified in [OCI runtime-spec](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md)
// Also check [runc commandline documentation](https://github.com/opencontainers/runc/blob/master/man/runc.8.md) for more explanation
#[derive(Parser, Debug)]
#[clap(version = "0.5.4alpha", author = env!("CARGO_PKG_AUTHORS"))]
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
//const WORKPATH: &str = "/usr/share/runPHI";
const RUNDIR: &str = "/run/runPHI";

fn main() -> Result<(), Box<dyn Error>> {
    //TODO: if no backend is available at the moment, forward to runc

    //let log_file = "/root/times.txt";
    //let mem_address = 0xFF250000;
    //let mem_size = 4096; // 4 KB (minimum granularity of mmap)
    //("start main", log_file, mem_address, mem_size).unwrap();

    let containerid;
    let mut config: serde_json::Value = serde_json::Value::Null;
    let opts = Opts::parse();
    //let _app = Opts::command();

    logging::init_logger(Some(std::path::PathBuf::from(std::path::Path::new("/usr/share/runPHI/log.txt"))));//opts.global.log);

    let _ = match opts.subcmd {
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
                logging::log_message(logging::Level::Info,  format!("Creating with id {}", &containerid).as_str());
                logging::log_message(logging::Level::Debug,  "Parse json");
                let config_json = fs::read_to_string(format!(
                    "{}/config.json",
                    &create.bundle.to_string_lossy().into_owned()))?;
                config = serde_json::from_str(&config_json)?;
                forwarding::runc_forward_ifnecessary(&config, &containerid);

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
                logging::log_message(logging::Level::Info,  format!("Starting with id {}", &containerid).as_str());
                forwarding::runc_forward_ifnecessary(&config, &containerid);
                let crundir = format!("{}/{}", RUNDIR, containerid);
                let _ = frontend::commands::start(&containerid, &crundir);
            }
            StandardCmd::Kill(kill) => {
                containerid = kill.container_id.chars().take(24).collect::<String>();
                logging::log_message(logging::Level::Info,  format!("Killing with id {}", &containerid).as_str());
                forwarding::runc_forward_ifnecessary(&config, &containerid);
                let crundir = format!("{}/{}", RUNDIR, containerid);
                let _ = frontend::commands::kill(&containerid, &crundir);
            }

            StandardCmd::Delete(delete) => {
                containerid = delete.container_id.chars().take(24).collect::<String>();
                logging::log_message(logging::Level::Info,  format!("Deleting with id {}", &containerid).as_str());
                forwarding::runc_forward_ifnecessary_delete(&config, &containerid);
                let crundir = format!("{}/{}", RUNDIR, containerid);
                let _ = frontend::commands::delete(&containerid, &crundir);
            }

            StandardCmd::State(state) => {
                containerid = state.container_id.chars().take(24).collect::<String>();
                logging::log_message(logging::Level::Info,  format!("State with id {}", &containerid).as_str());
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

    //log_timestamp_with_memory_mmap("end main", log_file, mem_address, mem_size).unwrap();
    return Ok(());
}

/* #[allow(dead_code)]
pub fn log_timestamp_with_memory_mmap(
    phase: &str,
    log_file: &str,
    mem_address: u64,
    mem_size: usize,
) -> std::io::Result<()> {
    // Get the current time in nanoseconds since UNIX epoch
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    let timestamp = now.as_secs() as u128 * 1_000_000_000 + now.subsec_nanos() as u128;

    // Open /dev/mem
    let file = std::fs::File::open("/dev/mem")?;
    let fd = file.as_raw_fd();

    // Map the memory region
    let mapped_addr = unsafe {
        mmap(
            std::ptr::null_mut(),
            mem_size,
            PROT_READ,
            MAP_SHARED,
            fd,
            mem_address as i64,
        )
    };

    if mapped_addr == nix::libc::MAP_FAILED {
        return Err(std::io::Error::last_os_error());
    }

    // Read the value at the memory address
    let value_at_address: u32 = unsafe { *(mapped_addr as *const u32) };

    // Unmap the memory region
    unsafe {
        munmap(mapped_addr, mem_size);
    }

    // Open the log file in append mode
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)?;

    // Write the phase, timestamp, and memory value to the file
    writeln!(
        file,
        "{}: {} ns, memory[0x{:X}] = 0x{:08X}",
        phase, timestamp, mem_address, value_at_address
    )?;

    Ok(())
} */