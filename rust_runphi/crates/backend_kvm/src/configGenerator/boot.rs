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

        // cpuconf() has already set os_arch
        let console = configGenerator::guest_console(&c.os_arch);
        let cmdline = if matches!(ic.disk_type.as_str(), "file" | "lvm") {
            format!("console={} root=/dev/vda rw", console)
        } else {
            format!("console={}", console)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn image(json: serde_json::Value) -> f2b::ImageConfig {
        serde_json::from_value(json).unwrap()
    }

    fn cmdline(arch: &str, ic: &f2b::ImageConfig) -> String {
        let mut c = configGenerator::BackendConfig::new();
        c.os_arch = arch.to_string();
        bootconf(ic, &mut c, &true).unwrap();
        c.os_boot_xml
    }

    #[test]
    fn test_bootconf_console_per_arch() {
        let ic = image(serde_json::json!({"os_var": "linux", "inmate": "/boot/Image"}));
        assert!(cmdline("aarch64", &ic).contains("<cmdline>console=ttyAMA0</cmdline>"));
        assert!(cmdline("x86_64", &ic).contains("<cmdline>console=ttyS0,115200</cmdline>"));
    }

    #[test]
    fn test_bootconf_root_disk() {
        let ic = image(serde_json::json!({
            "os_var": "linux",
            "inmate": "/boot/Image",
            "disk_type": "lvm"
        }));
        assert!(
            cmdline("aarch64", &ic).contains("<cmdline>console=ttyAMA0 root=/dev/vda rw</cmdline>")
        );
    }
}
