use std::error::Error;
use std::fs::{self};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::str;

use std::time::{Duration, Instant};
use std::thread::sleep;

use std::io::Write;


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

pub fn createguest(fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> {


    let conffile = fc.crundir.join("qemu.args");
    let qemu_log = fc.crundir.join("qemu.log");
    
    let qmp_socket_path = fc.crundir.join(format!("{}-qmp.sock", fc.containerid));
    
    let args_content = fs::read_to_string(&conffile)?;
    
    let qemu_args: Vec<&str> = args_content
        .lines()
        .map(|line| line.trim()) // Rimuove eventuali spazi bianchi o carriage return (\r)
        .filter(|line| !line.is_empty())
        .collect();

    let log_out = fs::File::create(&qemu_log)?;
    let log_err = log_out.try_clone()?;

    let qemu_child = Command::new("qemu-system-x86_64")
        .args(&qemu_args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_out))
        .stderr(Stdio::from(log_err))
        .spawn()?;

    std::fs::write(&fc.pidfile, format!("{}", qemu_child.id()))?;


    Ok(())
}

pub fn startguest(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    let qmp_socket = crundir.join(format!("{}-qmp.sock", containerid));

    let timeout = Duration::from_secs(3);
    let start_wait = Instant::now();

    while !qmp_socket.exists() {
        if start_wait.elapsed() > timeout {
            return Err(format!(
                "Timeout: QMP socket non trovato dopo 3s in: {}",
                qmp_socket.display()
            ).into());
        }

        sleep(Duration::from_millis(50));
    }

    let qmp_payload = "{\"execute\": \"qmp_capabilities\"}\n{\"execute\": \"cont\"}\n";
    

    let mut child = Command::new("socat")
        .arg("-")
        .arg(format!("UNIX-CONNECT:{}", qmp_socket.display()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Invia i comandi QMP allo standard input di socat
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(qmp_payload.as_bytes())?;
    }

    let output = child.wait_with_output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Errore socat durante l'avvio del guest: {}", stderr).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Verifica che QEMU abbia risposto positivamente al comando 'cont'
    if !stdout.contains("{\"return\": {}}") {
        return Err(format!("Risposta inattesa da QMP: {}", stdout).into());
    }

    Ok(())
}

pub fn stopguest(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    let qmp_socket = crundir.join(format!("{}-qmp.sock", containerid));

    if !qmp_socket.exists() {
        return Err(format!("QMP socket non trovato in: {}", qmp_socket.display()).into());
    }

    // Handshake QMP e invio del comando 'quit' per terminare QEMU
    let qmp_payload = "{\"execute\": \"qmp_capabilities\"}\n{\"execute\": \"stop\"}\n";

    let mut child = Command::new("socat")
        .arg("-")
        .arg(format!("UNIX-CONNECT:{}", qmp_socket.display()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(qmp_payload.as_bytes())?;
    }

    let output = child.wait_with_output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Errore socat durante lo stop del guest: {}", stderr).into());
    }

    Ok(())
}

pub fn destroyguest(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    
    let qmp_socket = crundir.join(format!("{}-qmp.sock", containerid));

    if !qmp_socket.exists() {
        return Err(format!("QMP socket non trovato in: {}", qmp_socket.display()).into());
    }

    // Handshake QMP e invio del comando 'quit' per terminare QEMU
    let qmp_payload = "{\"execute\": \"qmp_capabilities\"}\n{\"execute\": \"quit\"}\n";
    

    let mut child = Command::new("socat")
        .arg("-")
        .arg(format!("UNIX-CONNECT:{}", qmp_socket.display()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(qmp_payload.as_bytes())?;
    }

    let output = child.wait_with_output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Errore socat durante il quit del guest: {}", stderr).into());
    }

    // Pulizia facoltativa: rimuove il socket UNIX una volta terminato QEMU
    if qmp_socket.exists() {
        let _ = std::fs::remove_file(&qmp_socket);
    }

    fs::remove_dir_all(crundir).ok();

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
