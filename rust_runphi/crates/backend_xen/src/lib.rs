//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

use nix::sys::signal::Signal;
use nix::unistd::Pid;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::process::{Command, Output};
use std::str;
//use std::fs::OpenOptions;
use std::io::Write;
//use std::time::Instant; //TIME CLOCK MONOTONIC

use f2b::paths::CARONTE_BIN;

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

//const WORKPATH: &str = "/usr/share/runPHI";


pub fn startguest(containerid: &str, _crundir: &str) -> Result<(), Box<dyn Error>> {
    run_command(Command::new("xl").arg("unpause").arg(containerid))?;
    Ok(())
}

pub fn stopguest(containerid: &str, _crundir: &str) -> Result<(), Box<dyn Error>> {
    run_command(Command::new("xl").arg("pause").arg(containerid))?;
    Ok(())
}

//We need to implement a way to deassign the pci_devices (ivshmem) from a cell when we destroy it
//For now I'll put it here but it should be something that the jailhouse driver offers just as with the cpus
pub fn destroyguest(containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> {
    run_command(Command::new("xl").arg("destroy").arg(containerid))?;
    // Disk stuff, no disk in arm
    // TODO: for x86_64, we need to remove the disk from the vg
    // Construct the file path
    //let conffile = format!("{}/config.cfg", crundir);

    //let file = File::open(conffile.clone())?;
    //let reader = io::BufReader::new(file);

    // let mut disk = String::new();
    
    // let re_disk = Regex::new(r#"disk\s*=\s*\[\s*'(/dev/[^,]+)"#)
    //     .unwrap();
    
    // for line in reader.lines() {
    //     let line = line?; 

    //     if let Some(captures) = re_disk.captures(&line) {
    //         disk = captures.get(1).unwrap().as_str().to_string();
    //     }
    // }
    
    // //sudo lvremove /dev/vg_my_group/lv_my_volume
    // let _ = Command::new("lvremove")
    //     .arg(disk)
    //     .arg("-y")
    //     .output()
    //     .expect("Failed to execute command");


    //writeln!(logfile, "lib.rs after destroy")?; //DEBUG

    // Now kill caronte
    let pathtokill = std::fs::read_to_string(format!("{}/pidfile", crundir))?;
    let pidtokill = std::fs::read_to_string(pathtokill.trim())?;
    let pidk: i32 = pidtokill.trim().parse()?;
    let pid = Pid::from_raw(pidk);
    let _ = nix::sys::signal::kill(pid, Signal::SIGTERM);
    fs::remove_dir_all(crundir).ok();

    Ok(())
}

// pub fn cleanup(_containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> {
//     fs::remove_dir_all(&crundir).ok();
//     return Ok(());
// }

// Create spawns a process, caronte, that is required to keep the container open. Caronte is set as
// container init, and as long as containerd sees that is alive, the container is kept open
pub fn createguest(fc: &f2b::FrontendConfig, _ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> {
    
    //log_timestamp("Create_Guest_Start")?; //Just for boot times timeline extraction

    // Read bundle and pidfile paths from the filesystem
    let conffile = format!("{}/config.cfg", fc.crundir);

    // Disk stuff, no disk in arm
    //TODO: for x86_64, we need to manage the disk
    //let file = File::open(conffile.clone())?;
    //let reader = io::BufReader::new(file);

    //let mut storage_request = String::new();
    // let mut disk = String::new();

    // let re_st = Regex::new(r#"#storage_request\s*=\s*(\d+M)"#) // Es: #storage_request = 1024M
    //     .unwrap();
    // let re_disk = Regex::new(r#"disk\s*=\s*\[\s*'(/dev/[^,]+)"#)
    //     .unwrap();

    // for line in reader.lines() {
    //     let line = line?; 

    //     if let Some(captures) = re_st.captures(&line) {
    //         storage_request = captures.get(1).unwrap().as_str().to_string();
    //     }

    //     if let Some(captures) = re_disk.captures(&line) {
    //         disk = captures.get(1).unwrap().as_str().to_string();
    //     }
    // }

    // let mut parts = disk.rsplitn(2, '/');
    
    // // The firts part will be "name"
    // let name = parts.next().unwrap();
    
    // // The second one will be "/dev/gname"
    // let gname_path = parts.next().unwrap();

    // let _ = Command::new("lvcreate")
    //     .arg("-L")
    //     .arg(storage_request) 
    //     .arg("-n")
    //     .arg(name)
    //     .arg(gname_path)   
    //     .output()
    //     .expect("Error during vgs execution");

    // This is an asynchronous command, not good to take times
    /* let _ = Command::new("xl")
        .arg("create")
        .arg(conffile)
        .output()
        .expect("Failed to execute command");

    log_timestamp("Create_Guest_End")?; */

    // Launch xl create asynchronously
    let mut xl_process = Command::new("xl")
        .arg("create")
        .arg(conffile)
        .spawn()?;

    // Log immediately - this captures the moment the domain creation begins
    //log_timestamp("Create_Guest_End")?;

    // Wait for xl create to finish
    xl_process.wait()?;

    let command = "echo \"caronte is listening\"".to_string();

    let start_output = Command::new(CARONTE_BIN)
        .arg(command)
        .arg(&fc.containerid)
        .spawn()?;
    let pid = start_output.id();

    std::fs::write(&fc.pidfile, format!("{}", pid))?;

    //log_timestamp("Create_Guest_End")?; //Just for boot times timeline extraction

    Ok(())
}

pub fn storeinfo(fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> {
    std::fs::write(format!("{}/bundle", fc.crundir), &fc.bundle)?;
    std::fs::write(format!("{}/pidfile", fc.crundir), &fc.pidfile)?;
    std::fs::write(format!("{}/OS", fc.crundir), &ic.os_var)?;
    Ok(())
}


pub fn cleanup(_containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> {
    fs::remove_dir_all(crundir).ok();
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
//}

#[allow(dead_code)]
fn log_timestamp(message: &str) -> std::io::Result<()> {
    let timestamp = fs::read_to_string("/dev/arm_timer")?;
    let timestamp = timestamp.trim();
    
    // Append to your timestamp file
    let log_entry = format!("{} - {}\n", timestamp, message);
    
    // Use OpenOptions to append instead of overwriting
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/root/boot_times_raw_data.txt")?;
    
    file.write_all(log_entry.as_bytes())?;
    
    Ok(())
}