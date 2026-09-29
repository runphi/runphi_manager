use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::str;


#[allow(non_snake_case)]
pub mod configGenerator;
pub mod irq;
pub mod timer;
pub mod cgroups;

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

pub fn createguest(fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> {
    let domain_xml = fc.crundir.join("domain.xml");
    let domain_name = format!("runphi-{}", fc.containerid);

    // Provision disk if asked
    let diskstate = fc.crundir.join("disk");
    if diskstate.exists() {
        let state = fs::read_to_string(&diskstate)?;
        let mut parts = state.split_whitespace();
        let lv = parts.next().ok_or("malformed disk state file")?;
        let size_mb = parts.next().ok_or("malformed disk state file")?;

        if !Path::new(lv).exists() {
            if let Err(e) = configGenerator::disk::provision_lvm_root(lv, size_mb, &fc.mountpoint, &fc.crundir) {
                logging::log_message(
                    logging::Level::Info,
                    &format!("FATAL: provision_lvm_root failed: {}", e),
                );
                return Err(e);
            }
        }
    }

    run_command(Command::new("virsh").arg("create").arg(&domain_xml).arg("--paused"))?;

    let search_pattern = format!("qemu-system.*{}", domain_name);

    let pgrep_out = run_command(Command::new("pgrep") .arg("-f").arg(&search_pattern))?;

    let qemu_pid = String::from_utf8_lossy(&pgrep_out.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();

    logging::log_message(
        logging::Level::Info,
        &format!("PID trovato tramite pgrep: {}", qemu_pid),
    );

    let pid: u32 = qemu_pid
        .parse()
        .map_err(|e| format!("failed to parse QEMU pid '{}': {}", qemu_pid, e))?;

    // Setup cgroups for QEMU guest process to partition container resources
    if let Err(e) = cgroups::setup_cgroups(fc, ic, pid) {
        logging::log_message(
            logging::Level::Error,
            &format!("Failed to setup cgroups for container {}: {}", fc.containerid, e),
        );
        let _ = Command::new("virsh").arg("destroy").arg(&domain_name).output();
        return Err(e);
    }

    // NOTE(lorenzo): Start a small program which sees if qemu is killed. That is because 
    //                the real parent of qemu is libvirtd, not virsh. So when containerd tries to kill
    //                the PID written in the pidfile, the signal is sent to libvirtd, not containerd.
    //                containerd never knows if qemu got killed or no, so it leaves its state "Up"
    //                By using a watcher program, which dies if qemu is killed by a virsh destroy,
    //                containerd receives correctly the signal when the watcher dies. 
    let watcher = Command::new("sh")
        .arg("-c")
        .arg(format!("while [ -d /proc/{} ]; do sleep 0.2; done", qemu_pid))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    let watcher_pid = watcher.id().to_string();
    fs::write(&fc.pidfile, watcher_pid)?;

    // NOTE(lorenzo): Apply IRQ steering if specified
    if let Some(cpus) = irq::get_steer_irqs(fc, ic) {
        irq::apply_irq_steering(&fc.crundir, ic, &cpus)?;
    }

    Ok(())
}

pub fn startguest(containerid: &str, _crundir: &Path) -> Result<(), Box<dyn Error>> {
    let domain_name = format!("runphi-{}", containerid);
    run_command(Command::new("virsh").arg("resume").arg(&domain_name))?;
    Ok(())
}

pub fn stopguest(containerid: &str, _crundir: &Path) -> Result<(), Box<dyn Error>> {
    let domain_name = format!("runphi-{}", containerid);
    run_command(Command::new("virsh").arg("suspend").arg(&domain_name))?;
    Ok(())
}

pub fn destroyguest(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    let domain_name = format!("runphi-{}", containerid);

    // NOTE: Restore original IRQ affinities if they were steered
    if let Err(e) = irq::restore_irq_steering(crundir) {
        logging::log_message(
            logging::Level::Warn,
            &format!("Fallito ripristino affinità IRQ per '{}': {}", domain_name, e),
        );
    }

    let output = Command::new("virsh")
        .arg("destroy")
        .arg(&domain_name)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // NOTE(lorenzo): Ignore the warning if the guest was already down (ex. poweroff)
        if !stderr.contains("domain is not running") && !stderr.contains("Domain not found") {
            logging::log_message(
                logging::Level::Warn,
                &format!(
                    "virsh destroy per '{}' ha restituito: {}",
                    domain_name,
                    stderr.trim()
                ),
            );
        }
    }

    // teardown disk if it was present
    let diskstate = crundir.join("disk");
    if let Ok(state) = fs::read_to_string(&diskstate) {
        if let Some(lv) = state.split_whitespace().next() {
            match run_command(Command::new("lvremove").arg("-y").arg(lv)) {
                Ok(_) => {
                    let _ = fs::remove_file(&diskstate);
                }
                Err(e) => logging::log_message(
                    logging::Level::Warn,
                    &format!("could not remove LV {}: {}", lv, e),
                ),
            }
        }
    }

    // Teardown cgroups before crundir is removed
    if let Err(e) = cgroups::destroy_cgroups(containerid, crundir) {
        logging::log_message(
            logging::Level::Warn,
            &format!("Could not remove cgroup for container '{}': {}", domain_name, e),
        );
    }

    fs::remove_dir_all(crundir).ok();

    Ok(())
}

pub fn storeinfo(fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> {
    // bundle/pidfile are re-read with read_to_string by other commands and
    // parsed as path strings, so persist them as text rather than raw OsStr bytes.
    std::fs::write(
        fc.crundir.join("bundle"),
        fc.bundle.to_string_lossy().as_bytes(),
    )?;
    std::fs::write(
        fc.crundir.join("pidfile"),
        fc.pidfile.to_string_lossy().as_bytes(),
    )?;
    std::fs::write(fc.crundir.join("OS"), &ic.os_var)?;
    Ok(())
}

pub fn cleanup(_containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    fs::remove_dir_all(crundir).ok();
    Ok(())
}
