

use std::error::Error;
use std::process::Command;
use std::str;

use crate::configGenerator;
use crate::run_command;

// Emit the memory stanza, after checking the request against the
// hypervisor's free memory. Disk sizing used to live here too (it
// abused docker's --memory-reservation as a storage request and wrote
// #storage_request comments for lib.rs to regex back out); the root
// disk is now planned by disk.rs from boot/config.json fields.
pub fn memconf(
    c: &mut configGenerator::BackendConfig,
    ram_request: &u64,
) -> Result<(), Box<dyn Error>> {

    let output = run_command(Command::new("xl").arg("info"))?;
    let stdout = str::from_utf8(&output.stdout)?;

    let mut free_mb: Option<u64> = None;
    for line in stdout.lines() {
        if line.trim_start().starts_with("free_memory") {
            if let Some(value_str) = line.split(':').nth(1) {
                free_mb = value_str.trim().parse::<u64>().ok();
            }
        }
    }
    let free_mb = free_mb.ok_or("could not find free_memory in xl info output")?;
    logging::log_message(logging::Level::Info, &format!("Free memory: {} MB", free_mb));

    // 0 means no limit was set: fall back to a 1 GB guest.
    let ram = if *ram_request == 0 { 1024 } else { *ram_request };

    if ram > free_mb {
        logging::log_message(logging::Level::Error, "Not enough free memory for the guest.");
        return Err(format!(
            "not enough free memory: need {} MB, have {} MB",
            ram, free_mb
        )
        .into());
    }

    //Put the memory request in the config file
    c.conf.push_str(&format!(
        "\n#Initial memory and max memory\nmemory = {} \nmaxmem = {}\n",
        ram, ram
    ));

    Ok(())
}
