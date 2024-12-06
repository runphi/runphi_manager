//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (f.boccola@studenti.unina.it)
//*********************************************

use nix::sys::signal::Signal;
use nix::unistd::Pid;
use std::error::Error;
<<<<<<< HEAD
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
=======
use std::fs::{self};
//use std::io::Read;
use std::path::Path;
>>>>>>> d073669 (Updated to runphi 0.5.2 from branch refactoring)
use std::process::Command;
use std::str;
use toml::Value;
use std::time::Instant; //TIME CLOCK MONOTONIC

use f2b;
use logging;

#[allow(non_snake_case)]
pub mod configGenerator;

const WORKPATH: &str = "/usr/share/runPHI";
//const RUNDIR: &str = "/run/runPHI";
const JAILHOUSE_PATH: &str = "/root/jailhouse/tools/jailhouse";
const STATEFILE: &str = "state.toml";

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
            // Add the new memory segment to the list
            if let Some(memory) = memory {
                segments.push(Value::String(memory));
            }

<<<<<<< HEAD
    let end_address = phys_start + 0x10000 + size_2;

    Ok((phys_start, end_address))
}

// Version 1 - causes bug
/* fn extract_bdf(configuration: &str) -> i64 {
    // Find the second PCI device entry
    let networking_device_pattern = r#"{ /* IVSHMEM 00:01.0 (networking) */"#;
    if let Some(start_index) = configuration.find(networking_device_pattern) {
        let start_slice = &configuration[start_index..];
        let bdf_pattern = ".bdf = ";
        if let Some(bdf_index) = start_slice.find(bdf_pattern) {
            let bdf_slice = &start_slice[bdf_index + bdf_pattern.len()..];
            if let Some(end_index) = bdf_slice.find(',') {
                let bdf_expression = &bdf_slice[..end_index].trim();
                if let Some(shift_index) = bdf_expression.find("<<") {
                    let bdf_number = &bdf_expression[..shift_index].trim();
                    if let Ok(bdf) = bdf_number.parse::<i64>() {
                        return bdf;
=======
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

            // Sort segments by their start address
            parsed_segments.sort_by_key(|&(start, _)| start);

            // Merge contiguous segments
            let mut merged_segments = Vec::new();
            let mut current_segment = parsed_segments[0];

            for &(start, end) in &parsed_segments[1..] {
                if current_segment.1 == start {
                    // If contiguous, extend the current segment
                    current_segment.1 = end;
                } else {
                    // Otherwise, save the current segment and start a new one
                    merged_segments.push(current_segment);
                    current_segment = (start, end);
                }
            }
            // Add the last segment
            merged_segments.push(current_segment);

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
>>>>>>> d073669 (Updated to runphi 0.5.2 from branch refactoring)
                    }
                }
            }
        }
    }
<<<<<<< HEAD
    -1 // Default value if BDF extraction fails
} */
// Version 2, more streamlined, it just searches for the second occurrence of the .bdf field and takes the id
fn extract_bdf(configuration: &str) -> i64 {
    let mut bdf_count = 0;

    for line in configuration.lines() {
        if line.trim_start().starts_with(".bdf =") {
            bdf_count += 1;
            if bdf_count == 2 {
                if let Some(bdf_value) = line.split_whitespace().nth(2) {
                    if let Ok(bdf) = bdf_value.parse::<i64>() {
                        return bdf;
                    }
                }
            }
        }
    }

    -1 // Return -1 if the second .bdf line is not found or if parsing fails
}
=======
>>>>>>> d073669 (Updated to runphi 0.5.2 from branch refactoring)

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

<<<<<<< HEAD
    // Open the file in append mode, create it if it doesn't exist
    let mut timefile = OpenOptions::new()
    .create(true)
    .append(true)
    .open("/usr/share/runPHI/times_file.txt")?;
    
    // Write the message and current time to the file, separated by an equal sign
    writeln!(timefile, "{}", message)?;
    
    Ok(())
}

// Function to restore memory segment by adding phys_start and end_address to the free_segments.txt file
// Version 1, doesn't unify the different used memory segments, leading to bugs
/* fn restore_memory_segment(phys_start: u64, end_address: u64) -> io::Result<()> {
    let path = Path::new(WORKPATH).join(FREE_SEGMENTS_FILE);
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(&path)?;

    writeln!(file, "0x{:X}, 0x{:X}", phys_start, end_address)?;
=======
    // Save the updated state back to state.toml
    let updated_content = toml::to_string(&parsed_toml)?;
    fs::write(&file_path, updated_content)?;
>>>>>>> d073669 (Updated to runphi 0.5.2 from branch refactoring)

    Ok(())
} */

// Function to restore memory segment by restoring free_segments.txt file
// Version 2 also aggregates all contiguous memory segments 
fn restore_memory_segment(phys_start: u64, end_address: u64) -> io::Result<()> {
    let path = Path::new(WORKPATH).join(FREE_SEGMENTS_FILE);
    
    // Read the existing entries
    let mut segments = Vec::new();
    if let Ok(file) = File::open(&path) {
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line?;
            if let Some((start, end)) = parse_segment(&line) {
                segments.push((start, end));
            }
        }
    }

    // Add the new segment
    segments.push((phys_start, end_address));

    // Remove segments where start == end
    segments.retain(|&(start, end)| start != end);

    // Sort segments by start address
    segments.sort_unstable_by_key(|&(start, _)| start);

    // Merge contiguous segments
    let mut merged_segments = Vec::new();
    if let Some(mut current) = segments.first().cloned() {
        for &(start, end) in &segments[1..] {
            if current.1 == start {
                // Merge contiguous segments
                current.1 = end;
            } else {
                merged_segments.push(current);
                current = (start, end);
            }
        }
        merged_segments.push(current);
    }

    // Write the updated segments back to the file
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)?;

    for (start, end) in merged_segments {
        writeln!(file, "0x{:X}, 0x{:X}", start, end)?;
    }

    Ok(())
}

// Support function to restore_memory_segment
fn parse_segment(line: &str) -> Option<(u64, u64)> {
    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    if parts.len() == 2 {
        if let (Ok(start), Ok(end)) = (u64::from_str_radix(parts[0].trim_start_matches("0x"), 16), 
                                       u64::from_str_radix(parts[1].trim_start_matches("0x"), 16)) {
            return Some((start, end));
        }
    }
    None
}

pub fn startguest(containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> {
    //logging::log_message(logging::Level::Debug, format!("Start guest for cell with id {}", containerid).as_str());
    //let start = Instant::now(); //TAKE THE START TIME OF THE PHASE
    let os_content = std::fs::read_to_string(format!("{}/OS", crundir))?;
    let os = os_content.trim();
    if os == "linux" {
        println!("Linux non-root cell {} has already been running, connect to Guest through ssh root from localhost to port number exposed", containerid);
    } else {
        let _ = Command::new(JAILHOUSE_PATH)
            .arg("cell")
            .arg("start")
            .arg(containerid)
            .output()
            .expect("Failed to execute command");
    }
    //log_elapsed_time(start,"Duration of start cell"); //TAKE THE END TIME OF THE PHASE
    return Ok(());
}

pub fn stopguest(containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> {
    //let start_time = Instant::now();                                //TIME
    let _ = Command::new(JAILHOUSE_PATH)
        .arg("cell")
        .arg("shutdown")
        .arg(containerid)
        .output()
        .expect("Failed to execute command");
    // Now kill caronte
    let pathtokill = std::fs::read_to_string(format!("{}/pidfile", crundir))?;
    let pidtokill = std::fs::read_to_string(pathtokill.trim())?;
    let pidk: i32 = pidtokill.parse().expect("Failed to parse number");
    let pid = Pid::from_raw(pidk);
    let _ = nix::sys::signal::kill(pid, Signal::SIGTERM);
    //log_elapsed_time(start,"Duration of stop cell"); //TAKE THE END TIME OF THE PHASE

    return Ok(());
}

//We need to implement a way to deassign the pci_devices (ivshmem) from a cell when we destroy it
//For now I'll put it here but it should be something that the jailhouse driver offers just as with the cpus
pub fn destroyguest(containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> {

/*     //let start_time = Instant::now();                                 //TIME
    let configuration_path = format!("/run/runPHI/{}/config{}.conf", containerid, containerid);

    // Convert the file path to a PathBuf
    let path = PathBuf::from(configuration_path);

    // Open the file
    let mut file = File::open(&path)?;

    // Read the file contents into a string
    let mut configuration = String::new();
    file.read_to_string(&mut configuration)?;
 */
    let _ = destroy_update_state(containerid);

    // Execute the command to destroy the jailhouse cell using the name of the cell containerid
    let _ = Command::new(JAILHOUSE_PATH)
        .arg("cell")
        .arg("destroy")
        .arg(containerid)
        .output()
        .expect("Failed to execute command");

    // Now kill caronte
    let pathtokill = std::fs::read_to_string(format!("{}/pidfile", crundir))?;
    let pidtokill = std::fs::read_to_string(pathtokill.trim())?;
    let pidk: i32 = pidtokill.parse().expect("Failed to parse number");
    let pid = Pid::from_raw(pidk);
    let _ = nix::sys::signal::kill(pid, Signal::SIGTERM);
    fs::remove_dir_all(&crundir).ok();

    //log_elapsed_time(start,"Duration of destroy cell"); //TAKE THE END TIME OF THE PHASE
    return Ok(());
}

pub fn cleanup(_containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> {
    fs::remove_dir_all(&crundir).ok();
    return Ok(());
}

// Create spawns a process, caronte, that is required to keep the container open. Caronte is set as
// container init, and as long as containerd sees that is alive, the container is kept open
pub fn createguest(fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> {
    // Read bundle and pidfile paths from the filesystem
    let cellfile = format!("{}/{}.cell", fc.crundir, fc.containerid);

    // We have to differentiate among OSes, because linux has a different jh command
    // while other OSes may have special params, e.g. loading address for zephyr
    if ic.os_var == "linux" {
        // Determine the architecture
        // TODO: replace with smt better than lscpu
        let arch_output = std::process::Command::new("lscpu").output()?;
        let arch_lines = String::from_utf8_lossy(&arch_output.stdout);
        let arch: &str = arch_lines
            .lines()
            .nth(0)
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap();

        // Execute command based on architecture
        // We need an init process that starts monitoring and handles signals directed to partitioned container, move into shim??
        //TODO: not tested under x86. A patched kernel is needed
        match arch {
            "x86_64" => {
                let command = format!(
                    "jailhouse cell linux {} {} -i {} -c \"console=ttyS0,115200\"",
                    fc.containerid, ic.kernel, ic.initrd
                );
                let start_output = Command::new("/usr/share/runPHI/caronte")
                    .arg(command)
                    .arg(&fc.containerid)
                    .spawn()?;
                let pid = start_output.id();
                std::fs::write(&fc.pidfile, format!("{}", pid)).expect("Unable to write pidfile");
            }
            "aarch32" | "aarch64" => {
                let command = format!(
                    "jailhouse cell linux {} {} -d {} -i {} -c \"console ttyAMA0,115200\"",
                    fc.containerid, ic.kernel, ic.dtb, ic.cpio
                );
                let start_output = Command::new("/usr/share/runPHI/caronte")
                    .arg(command)
                    .arg(&fc.containerid)
                    .spawn()?;
                let pid = start_output.id();
                std::fs::write(&fc.pidfile, format!("{}", pid)).expect("Unable to write pidfile");
            }
            _ => {
                eprintln!("Arch not recognized");
                return Err("Arch not recognized".into());
            }
        }
    } else {

        // Handle baremetal or libOS built with application
        // Here we have to wait both commands to return to guarantee ordering, and then we start caronte
        // caronte is needed to keep a pid alive expected by containerd before giving the start
        
        //TODO: absolute path NOPE
        logging::log_message(logging::Level::Debug, format!("Creating cell on cellfile {}", &cellfile).as_str());
        //let start = Instant::now(); //TAKE THE START TIME OF THE PHASE

        Command::new(JAILHOUSE_PATH)
            .arg("cell")
            .arg("create")
            .arg(cellfile)
            .output()?;

        if !ic.starting_vaddress.is_empty() {
            logging::log_message(logging::Level::Debug, format!("Starting cell with id {} Vaddress specified", &fc.containerid).as_str());
            Command::new(JAILHOUSE_PATH)
                .arg("cell")
                .arg("load")
                .arg(&fc.containerid)
                .arg(ic.inmate.trim())
                .arg("-a")
                .arg(&ic.starting_vaddress)
                .output()?;
        } else {
            logging::log_message(logging::Level::Debug, format!("Starting cell with id {} Defaulting vaddress", &fc.containerid).as_str());
            Command::new(JAILHOUSE_PATH)
                .arg("cell")
                .arg("load")
                .arg(&fc.containerid)
                .arg(ic.inmate.trim())
                .output()?;
        }
        let command = format!("echo \"caronte is listening\"");
        logging::log_message(logging::Level::Debug, format!("Starting caronted with id {}", &fc.containerid).as_str());
        let start_output = Command::new("/usr/share/runPHI/caronte")
            .arg(command)
            .arg(&fc.containerid)
            .spawn()?;
        let pid = start_output.id();
        std::fs::write(&fc.pidfile, format!("{}", pid)).expect("Unable to write pidfile");
        //log_elapsed_time(start,"Duration of create cell"); //TAKE THE END TIME OF THE PHASE
    }
    Ok(())
}

pub fn storeinfo(fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> {
    std::fs::write(format!("{}/bundle", fc.crundir), &fc.bundle)?;
    std::fs::write(format!("{}/pidfile", fc.crundir), &fc.pidfile)?;
    std::fs::write(format!("{}/OS", fc.crundir), &ic.os_var)?;
    return Ok(());
}

// Function to log the elapsed time with a custom message
#[allow(dead_code)]
fn log_elapsed_time(start: Instant, message: &str) {
    // Calculate elapsed time from the provided start time
    let elapsed_ns = start.elapsed().as_nanos();

    // Log the elapsed time along with the message
    logging::log_message(
        logging::Level::Debug,
        &format!("{} :[{} ns]",  message , elapsed_ns),
    );
}
// pub fn storeadditionalinfo(c: &mut Backendconfig) -> Result<(), Box<dyn Error>> {
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
