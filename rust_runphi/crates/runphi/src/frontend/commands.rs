//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//*********************************************

use chrono;
use serde_json;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::backend;
use f2b;
use f2b::paths::WORKPATH;
use liboci_cli;

pub fn start(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    backend::startguest(containerid, crundir)
}

#[allow(dead_code)]
pub fn pause(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    backend::stopguest(containerid, crundir)
}

//UNIMPLEMENTED
#[allow(dead_code)]
pub fn resume(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    println!("{}", containerid);
    println!("{}", crundir.display());
    println!("{}", WORKPATH);
    Ok(())
}

#[allow(dead_code)]
pub fn stop(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    backend::stopguest(containerid, crundir)
}

// Flow: stop guest, destroy guest, look for processes (caronte and shim) containing the container id and kill em
pub fn kill(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    // Pause is best-effort: on a failed create there is no domain, so
    // `xl pause` errors — but destroyguest (xl destroy) tears down a running
    // or paused domain either way, so a pause failure must not abort teardown.
    let _ = backend::stopguest(containerid, crundir);
    backend::destroyguest(containerid, crundir)?;
    Ok(())
}

// Basically copy of destroy atm plus removal
pub fn delete(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    // Best-effort pause (see kill): ensures destroyguest — which removes the
    // /run/runPHI/<id> state dir — always runs, even when create failed and
    // left no domain to pause.
    let _ = backend::stopguest(containerid, crundir);
    backend::destroyguest(containerid, crundir)?;
    backend::cleanup(containerid, crundir)?;
    Ok(())
}

// Flow: call config generator to create config file, then call mount (?), create-guest giving the config file, and finally start guest
pub fn create(
    containerid: &str,
    args: liboci_cli::Create,
    crundir: &Path,
    parsedconfig: serde_json::Value,
) -> Result<(), Box<dyn Error>> {
    let mut f2b: f2b::FrontendConfig = f2b::FrontendConfig::new();
    f2b.crundir = crundir.to_path_buf();
    f2b.guestconsole = match args.console_socket {
        Some(console) => console,
        None => PathBuf::from("."),
    };
    f2b.containerid = containerid.to_string();
    f2b.bundle = args.bundle.clone();
    f2b.pidfile = args
        .pid_file
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "--pid-file is required"))?;
    f2b.jsonconfig = parsedconfig;

    //   OCI Bundle generation
    //TOOD: what is actually the purpose of this???
    if !crundir.join("bundle").exists() {
        let rootfs_in = f2b.jsonconfig["root"]["path"]
            .as_str()
            .ok_or_else(|| io::Error::other("Cannot determine rootfs"))?;
        let rootfs_path = Path::new(rootfs_in);
        f2b.mountpoint = if rootfs_path.is_absolute() {
            rootfs_path.to_path_buf()
        } else {
            args.bundle.join(rootfs_path)
        };
    }

    // Execute config_generator script to generate configuration file
    logging::log_message(logging::Level::Info,  format!("Creating config for ID {}", &containerid).as_str());
    let ic: f2b::ImageConfig = *backend::configGenerator::config_generate(&f2b)?;

    // Note: no explicit mount step here. containerd mounts the rootfs
    // before invoking our `create`, and ImageConfig::get_from_file reads
    // /boot/config.json from the already-mounted rootfs.

    logging::log_message(logging::Level::Info, format!("Creating guest for ID {}", &containerid).as_str());
    backend::createguest(&f2b, &ic)?;

    // Save info on files required by start guest as well as other commands
    // Here the point is that startguest maybe called alone, and it would read info from file
    // It is easier to not distinguish behavior and always read from file
    backend::storeinfo(&f2b, &ic)?;

    //backend::storeadditionalinfo(&mut backendconfig); Enable for debug

    Ok(())
}

//TODO: Test this, how to invoke from ctr???
pub fn state(container_id: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    // Read bundle and pidfile
    //TODO: move this to backend
    let bundle = fs::read_to_string(crundir.join("bundle"))?;
    let pidfile = fs::read_to_string(crundir.join("pidfile"))?;
    let mountpoint = fs::read_to_string(crundir.join("rootfs"))?;

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
    Ok(())
}
