//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (f.boccola@studenti.unina.it)
//*********************************************

use nix::sys::signal::Signal;
use nix::unistd::Pid;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;
//use std::time::Instant; //TIME CLOCK MONOTONIC

use f2b;
use logging;

#[allow(non_snake_case)]
pub mod configGenerator;

const WORKPATH: &str = "/usr/share/runPHI";
//const RUNDIR: &str = "/run/runPHI";
const PCI_IVSHMEM_ID_FILE: &str = "pci_ivshmem_id.txt";
const FREE_SEGMENTS_FILE: &str = "free_segments.txt";
const JAILHOUSE_PATH: &str = "/root/jailhouse/tools/jailhouse";

//Function to extract memory region to restore and bdf to deassign from the configuartion file of the container
fn extract_memory_bdf(configuration: &str) -> io::Result<(u64, u64, i64)> {
    // Extract memory size
    let (phys_start, end_address) = extract_memory_size(configuration)?;
    let mut bdf: i64 = -1;

    if configuration.contains("struct jailhouse_pci_device pci_devices[2]") {
        // Extract BDF only if there are two PCI devices
        bdf = extract_bdf(configuration);
    }

    Ok((phys_start, end_address, bdf))
}

//Support function to extract memory size
fn extract_memory_size(configuration: &str) -> io::Result<(u64, u64)> {
    let ram_region_pattern = r#"/* RAM */ {
            .phys_start"#;

    let phys_start: u64;
    let size_2: u64;

    // Find the first RAM region's phys_start
    if let Some(start_index) = configuration.find(ram_region_pattern) {
        let start_slice = &configuration[start_index..];
        if let Some(phys_start_index) = start_slice.find(".phys_start = 0x") {
            let phys_start_slice = &start_slice[phys_start_index + 16..];
            if let Some(end_index) = phys_start_slice.find(',') {
                let phys_start_hex = &phys_start_slice[..end_index].trim();
                if let Ok(phys_start_value) = u64::from_str_radix(phys_start_hex, 16) {
                    phys_start = phys_start_value;
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Invalid phys_start value",
                    ));
                }
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "End of phys_start not found",
                ));
            }
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "phys_start pattern not found",
            ));
        }
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "First RAM region pattern not found",
        ));
    }

    // Find the second RAM region's size
    if let Some(start_index) = configuration.rfind(ram_region_pattern) {
        let start_slice = &configuration[start_index..];
        if let Some(size_index) = start_slice.find(".size = 0x") {
            let size_slice = &start_slice[size_index + 9..];
            if let Some(end_index) = size_slice.find(',') {
                let size_hex = &size_slice[..end_index].trim();
                let size_hex = size_hex.trim_start_matches('x');
                if let Ok(size_value) = u64::from_str_radix(size_hex, 16) {
                    size_2 = size_value;
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Invalid size value",
                    ));
                }
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "End of size not found",
                ));
            }
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "size pattern not found",
            ));
        }
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Second RAM region pattern not found",
        ));
    }

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
                    }
                } else if let Ok(bdf) = bdf_expression.parse::<i64>() {
                    return bdf;
                }
            }
        }
    }
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

//Support function to deallocate a pci device by removing its bdf from the file
//TODO: bug here, is not always removed in a correct way ... if the file is already present without
//any started container, there is actually that cleans up the file, preventing the start of new
//container. This is because the assumption that the file always contains the bdf of running
//containers is violated.
fn remove_bdf(bdf: u8) -> io::Result<()> {
    let path = Path::new(WORKPATH).join(PCI_IVSHMEM_ID_FILE);
    let mut contents = String::new();

    // Read the file contents
    if path.exists() {
        let mut file = File::open(&path)?;
        file.read_to_string(&mut contents)?;
    }

    // Remove the specified bdf
    let new_contents: Vec<String> = contents
        .lines()
        .filter_map(|line| {
            let line_bdf: u8 = line.trim().parse().unwrap_or(255);
            if line_bdf != bdf {
                Some(line.to_string())
            } else {
                None
            }
        })
        .collect();

    // Write the updated contents back to the file
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    for line in new_contents {
        writeln!(file, "{}", line)?;
    }

    Ok(())
}

#[allow(dead_code)]
fn append_message_with_time(message: &str) -> Result<(), Box<dyn Error>> {

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
    //let start_time = Instant::now();                                //TIME
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
    //let _ = append_message_with_time(&format!("Time elapsed in Start guest is: {:?}", start_time.elapsed())); //TIME
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
    //let _ = append_message_with_time(&format!("Time elapsed in Stop guest is: {:?}", start_time.elapsed())); //TIME
    return Ok(());
}

//We need to implement a way to deassign the pci_devices (ivshmem) from a cell when we destroy it
//For now I'll put it here but it should be something that the jailhouse driver offers just as with the cpus
pub fn destroyguest(containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> {
    // Construct the file path
    //let start_time = Instant::now();                                 //TIME
    let configuration_path = format!("/run/runPHI/{}/config{}.conf", containerid, containerid);

    // Convert the file path to a PathBuf
    let path = PathBuf::from(configuration_path);

    // Open the file
    let mut file = File::open(&path)?;

    // Read the file contents into a string
    let mut configuration = String::new();
    file.read_to_string(&mut configuration)?;

    // Extract memory size and BDF (if applicable) based on the contents
    let (phys_start, end_address, bdf) = extract_memory_bdf(&configuration)?;

    // Call remove_bdf if bdf is valid
    if bdf != -1 {
        remove_bdf(bdf as u8)?;
    }

    // Always call restore_memory_segment
    restore_memory_segment(phys_start, end_address)?;

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

    //let _ = append_message_with_time(&format!("Time elapsed in Destroy guest is: {:?}", start_time.elapsed())); //TIME
    //let _ = append_message_with_time("");
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
        //let start_time = Instant::now();                                         //TIME
        logging::log_message(logging::Level::Debug, format!("Creating cell on cellfile {}", &cellfile).as_str());
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
        //let _ = append_message_with_time(&format!("Time elapsed in Create guest is: {:?}", start_time.elapsed())); //TIME
    }
    Ok(())
}

pub fn storeinfo(fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> {
    std::fs::write(format!("{}/bundle", fc.crundir), &fc.bundle)?;
    std::fs::write(format!("{}/pidfile", fc.crundir), &fc.pidfile)?;
    std::fs::write(format!("{}/OS", fc.crundir), &ic.os_var)?;
    return Ok(());
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
