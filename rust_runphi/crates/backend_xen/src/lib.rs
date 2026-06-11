//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

use nix::sys::signal::Signal;
use nix::unistd::Pid;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::path::Path;
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

// Create the per-container logical volume and populate it with a clone
// of the container rootfs, so the docker image becomes the guest's
// persistent root filesystem: lvcreate -> mkfs.ext4 -> mount -> copy ->
// umount. On any failure after lvcreate the LV is removed again so a
// failed create leaves no leaked volume behind.
fn provision_lvm_root(
    lv: &str,
    size_mb: &str,
    rootfs: &Path,
    crundir: &Path,
) -> Result<(), Box<dyn Error>> {
    // "/dev/<vg>/lv_<id>" -> lv name + vg name for lvcreate
    let mut comps = lv.rsplitn(3, '/');
    let lvname = comps.next().ok_or("malformed lv path")?;
    let vg = comps.next().ok_or("malformed lv path")?;

    run_command(
        Command::new("lvcreate")
            .arg("-L")
            .arg(format!("{}M", size_mb))
            .arg("-n")
            .arg(lvname)
            .arg(vg),
    )?;

    let mnt = crundir.join("mnt");
    let populate = (|| -> Result<(), Box<dyn Error>> {
        run_command(Command::new("mkfs.ext4").arg("-q").arg("-F").arg(lv))?;
        fs::create_dir_all(&mnt)?;
        run_command(Command::new("mount").arg(lv).arg(&mnt))?;
        // Always try to umount, even if the copy failed, then report the
        // first error of the two.
        let copied = run_command(
            Command::new("cp")
                .arg("-a")
                .arg(format!("{}/.", rootfs.display()))
                .arg(&mnt),
        );
        let unmounted = run_command(Command::new("umount").arg(&mnt));
        copied?;
        unmounted?;
        Ok(())
    })();

    if let Err(e) = populate {
        // Best-effort cleanup; the original error is what gets reported.
        let _ = Command::new("umount").arg(&mnt).output();
        let _ = Command::new("lvremove").arg("-y").arg(lv).output();
        return Err(e);
    }
    Ok(())
}

pub fn startguest(containerid: &str, _crundir: &Path) -> Result<(), Box<dyn Error>> {
    run_command(Command::new("xl").arg("unpause").arg(containerid))?;
    Ok(())
}

pub fn stopguest(containerid: &str, _crundir: &Path) -> Result<(), Box<dyn Error>> {
    run_command(Command::new("xl").arg("pause").arg(containerid))?;
    Ok(())
}

//We need to implement a way to deassign the pci_devices (ivshmem) from a cell when we destroy it
//For now I'll put it here but it should be something that the jailhouse driver offers just as with the cpus
pub fn destroyguest(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    run_command(Command::new("xl").arg("destroy").arg(containerid))?;

    // Remove the LVM-backed root disk, if one was provisioned for this
    // container (state file written by configGenerator::disk). Failure is
    // logged but not propagated: kill and delete both come through here,
    // and the volume may already be gone on the second pass.
    let diskstate = crundir.join("disk");
    if let Ok(state) = std::fs::read_to_string(&diskstate) {
        if let Some(lv) = state.split_whitespace().next() {
            match run_command(Command::new("lvremove").arg("-y").arg(lv)) {
                Ok(_) => {
                    let _ = fs::remove_file(&diskstate);
                }
                Err(e) => logging::log_message(
                    logging::Level::Warn,
                    format!("could not remove LV {}: {}", lv, e).as_str(),
                ),
            }
        }
    }

    //writeln!(logfile, "lib.rs after destroy")?; //DEBUG

    // Now kill caronte
    let pathtokill = std::fs::read_to_string(crundir.join("pidfile"))?;
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
    let conffile = fc.crundir.join("config.cfg");

    // Provision the LVM-backed root disk if the config generator planned
    // one (disk_type=="lvm"): the state file holds "<lv_path> <size_mb>".
    // The file-backed mode needs no provisioning (the image is used in
    // place), and bare-metal / initramfs guests have no state file at all.
    let diskstate = fc.crundir.join("disk");
    if diskstate.exists() {
        let state = std::fs::read_to_string(&diskstate)?;
        let mut parts = state.split_whitespace();
        let lv = parts.next().ok_or("malformed disk state file")?;
        let size_mb = parts.next().ok_or("malformed disk state file")?;
        provision_lvm_root(lv, size_mb, &fc.mountpoint, &fc.crundir)?;
    }

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
        .arg(&conffile)
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
    // bundle/pidfile are re-read with read_to_string by other commands and
    // parsed as path strings, so persist them as text rather than raw OsStr bytes.
    std::fs::write(fc.crundir.join("bundle"), fc.bundle.to_string_lossy().as_bytes())?;
    std::fs::write(fc.crundir.join("pidfile"), fc.pidfile.to_string_lossy().as_bytes())?;
    std::fs::write(fc.crundir.join("OS"), &ic.os_var)?;
    Ok(())
}


pub fn cleanup(_containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
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