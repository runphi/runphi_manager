//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

use nix::sys::signal::Signal;
use nix::unistd::Pid;
use std::error::Error;
use std::fs::{self};
use std::os::unix::fs::symlink;
//use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::str;
use toml::Value;
//use std::time::Instant; //TIME CLOCK MONOTONIC


#[allow(non_snake_case)]
pub mod configGenerator;
pub mod timer;

// Run an external command and return Err if it can't be spawned or
// exits non-zero. Replaces the .output().expect("Failed to execute
// command") pattern: the previous form panicked the entire runphi
// process on spawn failure and silently ignored non-zero exits.
fn run_command(cmd: &mut Command) -> Result<Output, Box<dyn Error>> {
    let prog = cmd.get_program().to_string_lossy().into_owned();
    let out = cmd
        .output()
        .map_err(|e| format!("failed to spawn {}: {}", prog, e))?;
    logging::log_message(
        logging::Level::Trace,
        &format!(
            "{} exited {:?}, stdout={:?}, stderr={:?}",
            prog,
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        ),
    );
    if !out.status.success() {
        return Err(format!(
            "{} failed (exit {}): {}",
            prog,
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).trim(),
        )
        .into());
    }
    Ok(out)
}

use f2b::paths::{CARONTE_BIN, WORKPATH};

const JAILHOUSE_PATH: &str = "/root/jailhouse/tools/jailhouse";
const STATEFILE: &str = "state.toml";
const FIRMWARE_DIR: &str = "/lib/firmware";

// Reads the state file and returns the rcpus string for the given containerid.
fn get_rcpu_for_container(containerid: &str) -> Result<String, Box<dyn Error>> {
    let file_path = Path::new(WORKPATH).join(STATEFILE);
    let content = fs::read_to_string(&file_path)?;
    let parsed: Value = content.parse::<Value>()?;
    if let Some(container) = parsed.get(containerid) {
        if let Some(rcpu) = container.get("rcpus").and_then(|v| v.as_str()) {
            return Ok(rcpu.to_string());
        }
    }
    Err(format!("rcpus for container {} not found", containerid).into())
}

fn destroy_update_state(containerid: &str) -> Result<(), Box<dyn Error>> {
    // Load and parse the current state from state.toml
    let file_path = Path::new(WORKPATH).join(STATEFILE);
    let content = fs::read_to_string(&file_path)?;
    let mut parsed_toml: Value = content.parse::<Value>()?;

    // Extract the data we need from the container section, if it exists
    let (memory, rcpus, pci_bdf) = if let Some(container) = parsed_toml.get(containerid) {
        (
            container.get("memory").and_then(|m| m.as_str()).map(String::from),
            container.get("rcpus").and_then(|r| r.as_str()).map(String::from),
            container.get("pci_bdf").and_then(|p| p.as_str()).map(String::from),
        )
    } else {
        return Err(format!("Container {} not found in state.toml", containerid).into());
    };

    // Free memory: Add container's memory segment back to `free_segments` +
    // + Merging logic for free memory segments
    if let Some(free_segments) = parsed_toml.get_mut("free_segments") {
        let segments = free_segments.get_mut("segments").and_then(|s| s.as_array_mut());
        
        if let Some(segments) = segments {
            // Add the new memory segment to the list ("none": an R5 cell, whose
            // memory is its core's region, not the pool's)
            if let Some(memory) = memory {
                if memory != "none" {
                    segments.push(Value::String(memory));
                }
            }

            // Parse segments into tuples of (start, end)
            let mut parsed_segments: Vec<(u64, u64)> = segments
                .iter()
                .filter_map(|s| s.as_str())
                .filter_map(|seg| {
                    let parts: Vec<&str> = seg.split(", ").collect();
                    if parts.len() == 2 {
                        Some((
                            u64::from_str_radix(parts[0].trim_start_matches("0x"), 16).ok()?,
                            u64::from_str_radix(parts[1].trim_start_matches("0x"), 16).ok()?,
                        ))
                    } else {
                        None
                    }
                })
                .collect();

            // Sort segments by their start address (empty ones dropped)
            parsed_segments.retain(|&(start, end)| start < end);
            parsed_segments.sort_by_key(|&(start, _)| start);

            // Merge contiguous segments
            let mut merged_segments: Vec<(u64, u64)> = Vec::new();
            for (start, end) in parsed_segments {
                match merged_segments.last_mut() {
                    // If contiguous, extend the current segment
                    Some(current) if current.1 == start => current.1 = end,
                    // Otherwise, start a new one
                    _ => merged_segments.push((start, end)),
                }
            }

            // Convert merged segments back to string format and update `segments`
            *segments = merged_segments
                .into_iter()
                .map(|(start, end)| Value::String(format!("0x{:x}, 0x{:x}", start, end)))
                .collect();
        }
    }


    // Free rcpus: Add container's `rcpus` back to `free_rcpus`
    if let Some(rcpus) = rcpus {
        if rcpus != "none" {
            if let Some(free_rcpus) = parsed_toml.get_mut("free_rcpus") {
                let ids = free_rcpus.get_mut("ids").and_then(|i| i.as_array_mut());
                if let Some(ids) = ids {
                    for rcpu in rcpus.split(',').map(|s| s.trim()) {
                        if let Ok(rcpu_value) = rcpu.parse::<i64>() {
                            ids.push(Value::Integer(rcpu_value));
                        }
                    }
                }
            }
        }
    }

    // Free pci_bdf: Add container's `pci_bdf` back to `free_pci_devices_bdf`
    if let Some(pci_bdf) = pci_bdf {
        if pci_bdf != "none" {
            if let Ok(bdf_value) = pci_bdf.parse::<i64>() {
                if let Some(free_pci_devices_bdf) = parsed_toml.get_mut("free_pci_devices_bdf") {
                    let bdf = free_pci_devices_bdf.get_mut("bdf").and_then(|b| b.as_array_mut());
                    if let Some(bdf) = bdf {
                        bdf.push(Value::Integer(bdf_value));
                    }
                }
            }
        }
    }

    // Remove the container section
    parsed_toml.as_table_mut().unwrap().remove(containerid);

    // Remove containerid from `[containerid].ids`
    if let Some(containerid_section) = parsed_toml.get_mut("containerid") {
        if let Some(ids) = containerid_section.get_mut("ids").and_then(|ids| ids.as_array_mut()) {
            ids.retain(|id| id.as_str() != Some(containerid));
        }
    }

    // Save the updated state back to state.toml
    let updated_content = toml::to_string(&parsed_toml)?;
    fs::write(&file_path, updated_content)?;

    Ok(())
}

// Marker in crundir: this container's create made the cell. runPHI only shuts
// down or destroys a cell it made: a cell of the same name made by someone
// else (jailhouse cell create by hand) is left alone.
const CELL_MARKER: &str = "cell";

fn owns_cell(containerid: &str, crundir: &Path) -> bool {
    crundir.join(CELL_MARKER).exists() && cell_exists(containerid)
}

// Is there a Jailhouse cell with this name? (none when Jailhouse is disabled)
fn cell_exists(name: &str) -> bool {
    let Ok(cells) = fs::read_dir("/sys/devices/jailhouse/cells") else {
        return false;
    };
    cells.filter_map(|cell| cell.ok()).any(|cell| {
        fs::read_to_string(cell.path().join("name"))
            .map(|n| n.trim() == name)
            .unwrap_or(false)
    })
}

// Name, in /lib/firmware, of the link to a container's R5 program: one per
// container (container ids are at most 24 characters, the driver takes names
// of up to 31), recorded in crundir/firmware so that destroyguest removes it.
fn firmware_link_name(containerid: &str) -> String {
    format!("runphi-{}", containerid)
}

fn remove_firmware_link(crundir: &Path) {
    if let Ok(name) = fs::read_to_string(crundir.join("firmware")) {
        let link = Path::new(FIRMWARE_DIR).join(name.trim());
        if fs::symlink_metadata(&link).is_ok() {
            let _ = fs::remove_file(&link);
        }
    }
}

// Send SIGTERM to the container's caronte, if it was started (its pid is in the
// file named by crundir/pidfile).
fn kill_caronte(crundir: &Path) {
    let pid = fs::read_to_string(crundir.join("pidfile"))
        .ok()
        .and_then(|pidfile| fs::read_to_string(pidfile.trim()).ok())
        .and_then(|pid| pid.trim().parse::<i32>().ok());
    if let Some(pid) = pid {
        let _ = nix::sys::signal::kill(Pid::from_raw(pid), Signal::SIGTERM);
    }
}

pub fn startguest(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    logging::log_message(logging::Level::Debug, format!("Start guest for cell with id {}", containerid).as_str());
    //let start = timer::capture(); //TIMELINE
    let os_content = std::fs::read_to_string(crundir.join("OS"))?;
    let os = os_content.trim();
    if os == "linux" {
        println!("Linux non-root cell {} has already been running, connect to Guest through ssh root from localhost to port number exposed", containerid);
    } else {
        let command_str = format!("{} {} {} {}", JAILHOUSE_PATH, "cell", "start", containerid);
        logging::log_message(logging::Level::Debug, format!("Starting cell with id {}", containerid).as_str());
        logging::log_message(logging::Level::Trace, format!("Starting cell by calling: {}", &command_str).as_str());
        run_command(
            Command::new(JAILHOUSE_PATH)
                .arg("cell")
                .arg("start")
                .arg(containerid),
        )?;
    }

    Ok(())
}

// Shut the cell down (if there is one) and stop caronte. A container whose
// create failed has no cell: nothing to shut down.
pub fn stopguest(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {

    //let start = timer::capture(); //TIMELINE

    if owns_cell(containerid, crundir) {
        let command_str = format!("{} {} {} {}", JAILHOUSE_PATH, "cell", "shutdown", containerid);
        logging::log_message(logging::Level::Trace, format!("The command is: {}", &command_str).as_str());
        run_command(
            Command::new(JAILHOUSE_PATH)
                .arg("cell")
                .arg("shutdown")
                .arg(containerid),
        )?;
    }
    kill_caronte(crundir);

    //let end = timer::capture(); //TIMELINE
    //timer::log_elapsed(start, end, "Cell stop")?;

    Ok(())
}

// Destroy the cell and give its resources back. It can be repeated and works on
// whatever a failed create left: a missing cell, state entry, pidfile or link
// is skipped. Only a cell that exists but cannot be destroyed is an error, and
// then its resources stay taken in state.toml.
//TODO: We need to implement a way to deassign the pci_devices (ivshmem) from a cell when we destroy it
//For now I'll put it here but it should be something that the jailhouse driver offers just as with the cpus
pub fn destroyguest(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {

    //let start = timer::capture(); //TIMELINE

    if owns_cell(containerid, crundir) {
        let command_str = format!("{} {} {} {}", JAILHOUSE_PATH, "cell", "destroy", containerid);
        logging::log_message(logging::Level::Trace, format!("The command is: {}", &command_str).as_str());
        run_command(
            Command::new(JAILHOUSE_PATH)
                .arg("cell")
                .arg("destroy")
                .arg(containerid),
        )?;
    }

    if let Err(e) = destroy_update_state(containerid) {
        logging::log_message(logging::Level::Debug, format!("No resources to give back for {}: {}", containerid, e).as_str());
    }
    remove_firmware_link(crundir);
    kill_caronte(crundir);
    fs::remove_dir_all(crundir).ok();

    //let end = timer::capture(); //TIMELINE
    //timer::log_elapsed(start, end, "Cell destroy")?;

    Ok(())
}

pub fn cleanup(_containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    fs::remove_dir_all(crundir).ok();
    Ok(())
}

// Undo what create did before `err`: destroy the cell if it exists, give the
// resources back, remove the firmware link. The frontend removes crundir.
fn abort_create(containerid: &str, crundir: &Path, err: Box<dyn Error>) -> Box<dyn Error> {
    logging::log_message(logging::Level::Error, format!("Creating {} failed, undoing it: {}", containerid, err).as_str());
    if owns_cell(containerid, crundir) {
        let _ = run_command(Command::new(JAILHOUSE_PATH).arg("cell").arg("destroy").arg(containerid));
    }
    let _ = destroy_update_state(containerid);
    remove_firmware_link(crundir);
    err
}

// Create spawns a process, caronte, that is required to keep the container open. Caronte is set as
// container init, and as long as containerd sees that is alive, the container is kept open
pub fn createguest(fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> {
    create_cell(fc, ic).map_err(|e| abort_create(&fc.containerid, &fc.crundir, e))
}

fn create_cell(fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> {

    //let start: u32 = timer::capture(); //TIMELINE

    // Read bundle and pidfile paths from the filesystem
    let cellfile: PathBuf = fc.crundir.join(format!("{}.cell", fc.containerid));

    // We have to differentiate among OSes, because linux has a different jh command
    // while other OSes may have special params, e.g. loading address for zephyr
    if ic.os_var != "linux" {
        // Handle baremetal or libOS built with application
        // Here we have to wait both commands to return to guarantee ordering, and then we start caronte
        // caronte is needed to keep a pid alive expected by containerd before giving the start

        logging::log_message(logging::Level::Debug, format!("Creating cell on cellfile {}", cellfile.display()).as_str());
        run_command(Command::new(JAILHOUSE_PATH).arg("cell").arg("create").arg(&cellfile))?;
        fs::write(fc.crundir.join(CELL_MARKER), &fc.containerid)?;

        logging::log_message(logging::Level::Debug, format!("Creating cell with id {}", &fc.containerid).as_str());
        logging::log_message(logging::Level::Trace, format!("The parameter ic.inmate is {}", &ic.inmate).as_str());

        // An R5 program: the Omnivisor loads it from /lib/firmware onto the rCPU
        // allotted to the container (in the state file), through remoteproc,
        // at its link address. A per-container link avoids copying it.
        if ic.rpu_req {
            let link_name = firmware_link_name(&fc.containerid);
            let sym_destination = Path::new(FIRMWARE_DIR).join(&link_name);
            logging::log_message(logging::Level::Trace, format!("The symdest is {}", sym_destination.display()).as_str());

            // Get the dynamic rcpus value from the state file.
            let rcpu = get_rcpu_for_container(&fc.containerid)?;

            if fs::symlink_metadata(&sym_destination).is_ok() {
                fs::remove_file(&sym_destination)?;
            }
            symlink(&ic.inmate, &sym_destination)
                .map_err(|e| format!("cannot link {} to {}: {}", sym_destination.display(), ic.inmate, e))?;
            fs::write(fc.crundir.join("firmware"), &link_name)?;

            logging::log_message(
                logging::Level::Trace,
                format!("Loading cell by calling: {} cell load {} -r {} {}", JAILHOUSE_PATH, fc.containerid, link_name, rcpu).as_str(),
            );
            run_command(
                Command::new(JAILHOUSE_PATH)
                    .arg("cell")
                    .arg("load")
                    .arg(&fc.containerid)
                    .arg("-r")
                    .arg(&link_name)
                    .arg(&rcpu),
            )?;

        //If an RPU is not required, we load the .bin onto an APU, remapping the first available address
        //In this case we might want to specify the virtual load address to the hypervisor, so that
        //the memory can be properly set.
        } else {
            logging::log_message(logging::Level::Debug, format!("Loading cell with id {}", &fc.containerid).as_str());

            let mut cmd_load = Command::new(JAILHOUSE_PATH);
            cmd_load.arg("cell").arg("load").arg(&fc.containerid).arg(&ic.inmate);

            // Append the starting vaddress when present in the JSON
            //TODO: MANAGE OMNIVISOR CONTAINERS APU
            if !ic.starting_vaddress.is_empty() {
                cmd_load.arg("-a").arg(&ic.starting_vaddress);
            }
            run_command(&mut cmd_load)?;
        }

        //let caronte_command = format!("echo \"caronte is listening\"");
        let caronte_command = String::new();
        logging::log_message(logging::Level::Debug, format!("Starting caronted with id {}", &fc.containerid).as_str());
        // caronte is the container's init for Docker: it runs in the
        // container's network namespace (f2b::FrontendConfig::enter_network_namespace).
        let mut caronte = Command::new(CARONTE_BIN);
        caronte.arg(caronte_command).arg(&fc.containerid);
        fc.enter_network_namespace(&mut caronte)?;
        let mut child = caronte.spawn()?;
        if let Err(e) = std::fs::write(&fc.pidfile, format!("{}", child.id())) {
            let _ = child.kill();
            return Err(e.into());
        }

    } else if ic.os_var == "linux" {
        // Here we manage separately the (unlikely) linux case due to a dedicated jh command
        // Commands only for Linux aarch64
        //TODO: create a gitlab branch for x86
        let command = format!(
                    "jailhouse cell linux {} {} -d {} -i {} -c \"console ttyAMA0,115200\"",
                    fc.containerid, ic.kernel, ic.dtb, ic.cpio
        );
        let mut caronte = Command::new(CARONTE_BIN);
        caronte.arg(command).arg(&fc.containerid);
        fc.enter_network_namespace(&mut caronte)?;
        let start_output = caronte.spawn()?;
        let pid = start_output.id();
        std::fs::write(&fc.pidfile, format!("{}", pid))?;
    }
    //let end = timer::capture(); //TIMELINE
    //timer::log_elapsed(start, end, "Cell creation")?;
    Ok(())
}

pub fn storeinfo(fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> {
    // bundle/pidfile are re-read with read_to_string by other commands and
    // parsed as path strings, so persist them as text rather than raw OsStr bytes.
    std::fs::write(fc.crundir.join("bundle"), fc.bundle.to_string_lossy().as_bytes())?;
    std::fs::write(fc.crundir.join("pidfile"), fc.pidfile.to_string_lossy().as_bytes())?;
    std::fs::write(fc.crundir.join("OS"), &ic.os_var)?;
    Ok(())
}

// pub fn storeadditionalinfo(c: &mut BackendConfig) -> Result<(), Box<dyn Error>> {
//     if !c.dtb.is_empty() {
//         let mut file = fs::File::create(format!("{}/dtb", c.crundir)).expect("Failed to create dtb file");
//         writeln!(file, "{}", c.dtb).expect("Failed to write dtb path");
//     }
//     if !c.cpio.is_empty() {
//         let mut file = fs::File::create(format!("{}/cpio", c.crundir)).expect("Failed to create cpio file");
//         writeln!(file, "{}", c.cpio).expect("Failed to write cpio path");
//     }
//     if !c.initrd.is_empty() {
//         let mut file = fs::File::create(format!("{}/initrd", c.crundir)).expect("Failed to create initrd file");
//         writeln!(file, "{}", c.initrd).expect("Failed to write initrd path");
//     }
//     if !c.kernel.is_empty() {
//         let mut file = fs::File::create(format!("{}/kernel", c.crundir)).expect("Failed to create kernel file");
//         writeln!(file, "{}", c.kernel).expect("Failed to write kernel path");
//     }
//     return Ok(());
// }
