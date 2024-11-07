//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (f.boccola@studenti.unina.it)
//*********************************************

use nix::sys::signal::Signal;
use nix::unistd::Pid;
use regex::Regex;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufRead};
use std::process::Command;
use std::str;
//use std::fs::OpenOptions;
//use std::io::Write;
//use std::time::Instant; //TIME CLOCK MONOTONIC

use f2b;

#[allow(non_snake_case)]
pub mod configGenerator;

//const WORKPATH: &str = "/usr/share/runPHI";


pub fn startguest(containerid: &str, _crundir: &str) -> Result<(), Box<dyn Error>> {

    let _ = Command::new("xl")
        .arg("unpause")
        .arg(containerid)
        .output()
        .expect("Failed to execute command");
    
    return Ok(());
}

pub fn stopguest(containerid: &str, _crundir: &str) -> Result<(), Box<dyn Error>> {
    //let start_time = Instant::now();                                //TIME
    let _ =Command::new("xl")
        .arg("pause")
        .arg(containerid)
        .output()
        .expect("Failed to execute command");

    return Ok(());
}

//We need to implement a way to deassign the pci_devices (ivshmem) from a cell when we destroy it
//For now I'll put it here but it should be something that the jailhouse driver offers just as with the cpus
pub fn destroyguest(containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> {

    let _ = Command::new("xl")
        .arg("destroy")
        .arg(containerid)
        .output()
        .expect("Failed to execute command");
    // 
    // Construct the file path
    let conffile = format!("{}/config.cfg", crundir);

    let file = File::open(conffile.clone())?;
    let reader = io::BufReader::new(file);

    let mut disk = String::new();
    
    let re_disk = Regex::new(r#"disk\s*=\s*\[\s*'(/dev/[^,]+)"#)
        .unwrap();
    
    for line in reader.lines() {
        let line = line?; 

        if let Some(captures) = re_disk.captures(&line) {
            disk = captures.get(1).unwrap().as_str().to_string();
        }
    }
    
    //sudo lvremove /dev/vg_my_group/lv_my_volume
    let _ = Command::new("lvremove")
        .arg(disk)
        .arg("-y")
        .output()
        .expect("Failed to execute command");


    //writeln!(logfile, "lib.rs after destroy")?; //DEBUG

    // Now kill caronte
    let pathtokill = std::fs::read_to_string(format!("{}/pidfile", crundir))?;
    let pidtokill = std::fs::read_to_string(pathtokill.trim())?;
    let pidk: i32 = pidtokill.parse().expect("Failed to parse number");
    let pid = Pid::from_raw(pidk);
    let _ = nix::sys::signal::kill(pid, Signal::SIGTERM);
    fs::remove_dir_all(&crundir).ok();

    return Ok(());
}

// pub fn cleanup(_containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> {
//     fs::remove_dir_all(&crundir).ok();
//     return Ok(());
// }

// Create spawns a process, caronte, that is required to keep the container open. Caronte is set as
// container init, and as long as containerd sees that is alive, the container is kept open
pub fn createguest(fc: &f2b::FrontendConfig, _ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> {
    // Read bundle and pidfile paths from the filesystem
    let conffile = format!("{}/config.cfg", fc.crundir);

    let file = File::open(conffile.clone())?;
    let reader = io::BufReader::new(file);

    let mut storage_request = String::new();
    let mut disk = String::new();

    let re_st = Regex::new(r#"#storage_request\s*=\s*(\d+M)"#) // Es: #storage_request = 1024M
        .unwrap();
    let re_disk = Regex::new(r#"disk\s*=\s*\[\s*'(/dev/[^,]+)"#)
        .unwrap();

    for line in reader.lines() {
        let line = line?; 

        if let Some(captures) = re_st.captures(&line) {
            storage_request = captures.get(1).unwrap().as_str().to_string();
        }

        if let Some(captures) = re_disk.captures(&line) {
            disk = captures.get(1).unwrap().as_str().to_string();
        }
    }

    let mut parts = disk.rsplitn(2, '/');
    
    // The firts part will be "name"
    let name = parts.next().unwrap();
    
    // The second one will be "/dev/gname"
    let gname_path = parts.next().unwrap();

    let _ = Command::new("lvcreate")
        .arg("-L")
        .arg(storage_request) 
        .arg("-n")
        .arg(name)
        .arg(gname_path)   
        .output()
        .expect("Error during vgs execution");


    // The command is independent of the Linux architecture and the guest OS.
    let _ = Command::new("xl")
        .arg("create")
        .arg(conffile)
        .output()
        .expect("Failed to execute command");

    let command = format!("echo \"caronte is listening\"");

    let start_output = Command::new("/usr/share/runPHI/caronte")
        .arg(command)
        .arg(&fc.containerid)
        .spawn()?;
    let pid = start_output.id();

    std::fs::write(&fc.pidfile, format!("{}", pid)).expect("Unable to write pidfile");

    //writeln!(logfile, "lib.rs createguest end")?; //DEBUG
    Ok(())
}

pub fn storeinfo(fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> {
    std::fs::write(format!("{}/bundle", fc.crundir), &fc.bundle)?;
    std::fs::write(format!("{}/pidfile", fc.crundir), &fc.pidfile)?;
    std::fs::write(format!("{}/OS", fc.crundir), &ic.os_var)?;
    return Ok(());
}


pub fn cleanup(_containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> {
    fs::remove_dir_all(&crundir).ok();
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
//}


// fn append_message(message: &str) -> Result<(), Box<dyn Error>> {  //TIME

//     // Open the file in append mode, create it if it doesn't exist
//     let mut log_lib = OpenOptions::new()
//     .create(true)
//     .append(true)
//     .open("/usr/share/runPHI/log_lib.txt")?;
    
//     // Write the message and current time to the file, separated by an equal sign
//     writeln!(log_lib, "{}", message)?;
    
//     Ok(())
// }