use std::error::Error;

use crate::configGenerator;
use f2b;

pub fn bootconf(
    ic: &f2b::ImageConfig,
    c: &mut configGenerator::BackendConfig,
    is_linux: &bool,
) -> Result<(), Box<dyn Error>> {
    let mut os_boot = String::new();

    if *is_linux {
        if !ic.inmate.is_empty() {
            os_boot.push_str(&format!("    <kernel>{}</kernel>\n", ic.inmate));
        }
        if !ic.ramdisk.is_empty() {
            os_boot.push_str(&format!("    <initrd>{}</initrd>\n", ic.ramdisk));
        }
        if !ic.dtb.is_empty() {
            os_boot.push_str(&format!("    <dtb>{}</dtb>\n", ic.dtb));
        }

        let cmdline = if matches!(ic.disk_type.as_str(), "file" | "lvm") {
            "console=ttyS0,115200 root=/dev/vda rw"
        } else {
            "console=ttyS0,115200"
        };

        os_boot.push_str(&format!("    <cmdline>{}</cmdline>", cmdline));
    } else {
        // Payload Bare-Metal (Zephyr, unikernel, raw ELF)
        if !ic.inmate.is_empty() {
            os_boot.push_str(&format!("    <kernel>{}</kernel>", ic.inmate));
        }
    }

    c.os_boot_xml = os_boot;

    Ok(())
}
