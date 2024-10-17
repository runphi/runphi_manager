//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//*********************************************

use chrono;
use serde_json;
use std::fs;
use std::io;
use std::io::Error;
use std::path::Path;

use backend;
use f2b;
use liboci_cli;

const WORKPATH: &str = "/usr/share/runPHI";

pub fn start(containerid: &str, crundir: &str) {
    //TODO: check and handle return
    let _ = backend::startguest(containerid, crundir);
}

#[allow(dead_code)]
pub fn pause(containerid: &str, crundir: &str) {
    let _ = backend::stopguest(containerid, crundir);
    //TODO: check status
}

//UNIMPLEMENTED
#[allow(dead_code)]
pub fn resume(containerid: &str, crundir: &str) {
    println!("{}", containerid);
    println!("{}", crundir);
    println!("{}", WORKPATH);
}

#[allow(dead_code)]
pub fn stop(containerid: &str, crundir: &str) {
    let _ = backend::stopguest(containerid, crundir);
    //TODO: check status
}

// Flow: stop guest, destory guest, look for processes (caronte and shim) containing the container id and kill em
pub fn kill(containerid: &str, crundir: &str) {
    let _ = backend::stopguest(containerid, crundir);
    //TODO: check status

    let _ = backend::destroyguest(containerid, crundir);
    //TODO: check status
}

// Basically copy of destroy atm plus remotion
pub fn delete(containerid: &str, crundir: &str) {
    let _ = backend::stopguest(containerid, crundir);
    //TODO: check status

    let _ = backend::destroyguest(containerid, crundir);
    //TODO: check status

    let _ = backend::cleanup(containerid, crundir);
}

// Flow: call config generator to create config file, then call mount (?), create-guest giving the config file, and finally start guest
pub fn create(
    containerid: &str,
    args: liboci_cli::Create,
    crundir: &str,
    parsedconfig: serde_json::Value,
) -> Result<(), Error> {
    let mut f2b: f2b::FrontendConfig = f2b::FrontendConfig::new();
    //TODO: replace the following with something unaware of the backend
    f2b.crundir = crundir.to_string();
    f2b.guestconsole = match args.console_socket {
        Some(console) => console.to_string_lossy().into_owned(),
        None => ".".to_string(),
    };
    f2b.containerid = containerid.to_string();
    f2b.bundle = args.bundle.to_string_lossy().into_owned();
    f2b.pidfile = args.pid_file.unwrap().to_string_lossy().into_owned();
    f2b.jsonconfig = parsedconfig;

    //   OCI Bundle generation
    //TOOD: what is actually the purpose of this???
    if !Path::new(&format!("{}/bundle", &crundir)).exists() {
        let rootfs_in = f2b.jsonconfig["root"]["path"]
            .as_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Cannot determine rootfs"))?;
        f2b.mountpoint = if rootfs_in.starts_with('/') {
            rootfs_in.to_string()
        } else {
            //TODO: very very dirty here manage to handle path not with strings
            format!(
                "{}/{}",
                &args.bundle.to_string_lossy().into_owned(),
                rootfs_in
            )
        };
    }

    // Execute config_generator script to generate configuration file
    logging::log_message(logging::Level::Info,  format!("Creating config for ID {}", &containerid).as_str());
    let ic_p = backend::configGenerator::config_generate(&f2b);
    let ic: f2b::ImageConfig = *ic_p.unwrap();

    // Execute mount utility to adjust rootfs
    //TODO: call mount
    // let status = Command::new("sh")
    //     .arg("-c")
    //     .arg(format!("{}/backend/mount {} \"{}\" mount", WORKPATH, containerid, crundir))
    //     .status();

    logging::log_message(logging::Level::Info, format!("Creating guest for ID {}", &containerid).as_str());
    let _ = backend::createguest(&f2b, &ic);
    //TODO handle return value

    // Save info on files required by start guest as well as other commands
    // Here the point is that startguest maybe called alone, and it would read info from file
    // It is easier to not distinguish behavior and always read from file
    let _ = backend::storeinfo(&f2b, &ic);

    //backend::storeadditionalinfo(&mut backendconfig); Enable for debug

    return Ok(());
}

//TODO: Test this, how to invoke from ctr???
pub fn state(container_id: &str, crundir: &str) -> Result<(), Error> {
    // Read bundle and pidfile
    //TODO: move this to backend
    let bundle = fs::read_to_string(format!("{}/bundle", crundir))?;
    let pidfile = fs::read_to_string(format!("{}/pidfile", crundir))?;
    let mountpoint = fs::read_to_string(format!("{}/rootfs", crundir))?;

    // Read pid from pidfile or set to 1 if file does not exist
    let pid = if let Ok(pid) = fs::read_to_string(&pidfile) {
        pid.trim().parse::<i32>().unwrap_or(1)
    } else {
        1
    };

    // Get current date
    let date = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    // Print JSON output
    println!(
        r#"{{
      "ociVersion": "1.0.2-dev",
      "id": "{}",
      "pid": {},
      "status": "running",
      "bundle": "{}",
      "rootfs": "{}",
      "created": "{}",
      "owner": ""
    }}"#,
        container_id, pid, bundle, mountpoint, date
    );
    return Ok(());
}
