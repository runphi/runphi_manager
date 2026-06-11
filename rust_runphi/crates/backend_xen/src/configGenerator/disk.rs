//*********************************************
// Authors:
// Francesco Boccola (francesco.boccola@unina.it)
//*********************************************
//
// Root-disk stanza for a Linux domU. Three strategies, selected by
// `disk_type` in the container's /boot/config.json:
//
//   ""     no disk: root comes from the initramfs (ramdisk). Default,
//          and the only option for bare-metal guests.
//   "file" a raw ext4 image shipped in the container is attached in
//          place from the mounted rootfs. Zero host-side provisioning;
//          guest writes land in the container's writable layer and are
//          discarded with the container.
//   "lvm"  a logical volume is provisioned on the host at create time
//          and populated with a clone of the container rootfs, so the
//          docker image itself becomes the VM's root filesystem. The
//          LV is removed at destroy.
//
// This module only plans the disk and emits the config line; the
// actual lvcreate/mkfs/copy lifecycle runs in lib.rs (createguest /
// destroyguest), driven by the state file written here.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::configGenerator;
use crate::run_command;
use f2b;

// Default LVM volume group hosting guest root disks. Can be overridden
// per-host with a one-line file (see VG_OVERRIDE_FILE).
const DEFAULT_VG: &str = "test-vg";

// Optional host-side override for the volume group name: a plain text
// file containing just the VG name. Lives under the runPHI workdir so
// it travels with the host installation, not with container images.
const VG_OVERRIDE_FILE: &str = "/usr/share/runPHI/xen_lvm_vg";

// Slack added on top of the measured rootfs size when the image does
// not specify disk_size: ~30% growth room plus a fixed floor for
// filesystem metadata.
const SIZE_SLACK_NUM: u64 = 13;
const SIZE_SLACK_DEN: u64 = 10;
const SIZE_FLOOR_MB: u64 = 64;

// xl disk specification for a raw read-write root device. Both the
// file-backed and LVM-backed strategies attach as xvda; the matching
// root=/dev/xvda is emitted on the kernel command line by boot.rs.
fn disk_line(path: &str) -> String {
    format!("disk = ['{},raw,xvda,rw']\n", path)
}

// Device path of the per-container logical volume.
fn lv_path(vg: &str, containerid: &str) -> String {
    format!("/dev/{}/lv_{}", vg, containerid)
}

// LV size when the image does not specify one: rootfs size + slack.
fn default_size_mb(rootfs_mb: u64) -> u64 {
    rootfs_mb * SIZE_SLACK_NUM / SIZE_SLACK_DEN + SIZE_FLOOR_MB
}

// Volume group to allocate from: host override file, or the default.
fn vg_name() -> String {
    match fs::read_to_string(VG_OVERRIDE_FILE) {
        Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => DEFAULT_VG.to_string(),
    }
}

// Size of the container rootfs in MB, used to size and sanity-check
// the LV that will receive its clone.
fn rootfs_size_mb(mountpoint: &Path) -> Result<u64, Box<dyn Error>> {
    let out = run_command(Command::new("du").arg("-sxm").arg(mountpoint))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let first = stdout
        .split_whitespace()
        .next()
        .ok_or("empty du output for rootfs")?;
    Ok(first.parse::<u64>()?)
}

// Free space in the volume group, in MB.
fn vg_free_mb(vg: &str) -> Result<u64, Box<dyn Error>> {
    let out = run_command(
        Command::new("vgs")
            .arg("--noheadings")
            .arg("--nosuffix")
            .arg("--units")
            .arg("m")
            .arg("-o")
            .arg("vg_free")
            .arg(vg),
    )?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let free: f64 = stdout.trim().parse()?;
    Ok(free as u64)
}

pub fn diskconf(
    fc: &f2b::FrontendConfig,
    c: &mut configGenerator::BackendConfig,
    ic: &f2b::ImageConfig,
) -> Result<(), Box<dyn Error>> {
    match ic.disk_type.as_str() {
        // No disk: bare-metal guests and initramfs-only Linux guests.
        "" => Ok(()),

        "file" => {
            if ic.disk_image.is_empty() {
                return Err("disk_type=\"file\" requires disk_image in boot/config.json".into());
            }
            // disk_image was already resolved against the rootfs mountpoint
            // by ImageConfig::get_from_file, so it must exist on the host.
            if !Path::new(&ic.disk_image).exists() {
                return Err(format!(
                    "disk image {} not found in container rootfs",
                    ic.disk_image
                )
                .into());
            }
            logging::log_message(
                logging::Level::Info,
                format!("Attaching file-backed root disk {}", ic.disk_image).as_str(),
            );
            c.conf.push_str(&disk_line(&ic.disk_image));
            Ok(())
        }

        "lvm" => {
            let vg = vg_name();
            let rootfs_mb = rootfs_size_mb(&fc.mountpoint)?;
            let size_mb = if ic.disk_size == 0 {
                default_size_mb(rootfs_mb)
            } else {
                if ic.disk_size < rootfs_mb {
                    return Err(format!(
                        "disk_size {} MB is smaller than the container rootfs ({} MB)",
                        ic.disk_size, rootfs_mb
                    )
                    .into());
                }
                ic.disk_size
            };
            let free_mb = vg_free_mb(&vg)?;
            if size_mb > free_mb {
                return Err(format!(
                    "not enough free space in volume group {}: need {} MB, have {} MB",
                    vg, size_mb, free_mb
                )
                .into());
            }
            let lv = lv_path(&vg, &fc.containerid);
            logging::log_message(
                logging::Level::Info,
                format!("Planning LVM root disk {} ({} MB)", lv, size_mb).as_str(),
            );
            c.conf.push_str(&disk_line(&lv));
            // State file consumed by createguest (provision) and destroyguest
            // (lvremove). Structured replacement for the old approach of
            // regex-parsing #storage_request comments out of config.cfg.
            fs::write(fc.crundir.join("disk"), format!("{} {}", lv, size_mb))?;
            Ok(())
        }

        other => Err(format!(
            "unknown disk_type \"{}\" in boot/config.json (expected \"\", \"file\" or \"lvm\")",
            other
        )
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image_config(disk_type: &str, disk_image: &str) -> f2b::ImageConfig {
        let mut ic: f2b::ImageConfig = serde_json::from_str("{}").unwrap();
        ic.disk_type = disk_type.to_string();
        ic.disk_image = disk_image.to_string();
        ic
    }

    fn run_diskconf(ic: &f2b::ImageConfig) -> Result<String, Box<dyn Error>> {
        let fc = f2b::FrontendConfig::new();
        let mut c = configGenerator::BackendConfig::new();
        diskconf(&fc, &mut c, ic)?;
        Ok(c.conf)
    }

    #[test]
    fn line_helpers() {
        assert_eq!(
            disk_line("/dev/vg0/lv_abc"),
            "disk = ['/dev/vg0/lv_abc,raw,xvda,rw']\n"
        );
        assert_eq!(lv_path("vg0", "abc123"), "/dev/vg0/lv_abc123");
        // 1000 MB rootfs -> 1300 + 64 MB
        assert_eq!(default_size_mb(1000), 1364);
    }

    // disk_type "" (bare-metal and initramfs-only Linux) must not touch
    // the config: regression guard for existing guests.
    #[test]
    fn no_disk_type_is_a_noop() {
        let ic = image_config("", "");
        assert_eq!(run_diskconf(&ic).unwrap(), "");
    }

    #[test]
    fn file_requires_existing_image() {
        // Missing disk_image field
        let ic = image_config("file", "");
        assert!(run_diskconf(&ic).is_err());
        // disk_image set but the file does not exist
        let ic = image_config("file", "/nonexistent/rootfs.img");
        assert!(run_diskconf(&ic).is_err());
    }

    #[test]
    fn file_emits_disk_line_for_existing_image() {
        let img = std::env::temp_dir().join(format!("runphi_disk_test_{}.img", std::process::id()));
        fs::write(&img, b"fake").unwrap();
        let ic = image_config("file", img.to_str().unwrap());
        let conf = run_diskconf(&ic).unwrap();
        let _ = fs::remove_file(&img);
        assert_eq!(conf, disk_line(img.to_str().unwrap()));
    }

    #[test]
    fn unknown_disk_type_is_rejected() {
        let ic = image_config("nfs", "");
        assert!(run_diskconf(&ic).is_err());
    }
}
