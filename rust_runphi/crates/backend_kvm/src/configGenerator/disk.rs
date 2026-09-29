//***********************************************************
// Author: Carmine Colucci (carmi.colucci@studenti.unina.it)
//***********************************************************

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;

use f2b;
use crate::configGenerator::BackendConfig;
use crate::run_command;

// Default LVM volume group hosting guest root disks. Can be overridden
// per-host with a one-line file (see VG_OVERRIDE_FILE).
const DEFAULT_VG: &str = "test-vg";

// Optional host-side override for the volume group name: a plain text
// file containing just the VG name. Lives under the runPHI workdir so
// it travels with the host installation, not with container images.
const VG_OVERRIDE_FILE: &str = "/usr/share/runPHI/kvm_lvm_vg";

// Slack added on top of the measured rootfs size when the image does
// not specify disk_size: ~30% growth room plus a fixed floor for
// filesystem metadata.
const SIZE_SLACK_NUM: u64 = 13;
const SIZE_SLACK_DEN: u64 = 10;
const SIZE_FLOOR_MB: u64 = 64;

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
    c: &mut BackendConfig,
    ic: &f2b::ImageConfig,
) -> Result<(), Box<dyn Error>> {
    match ic.disk_type.as_str() {
        // Bare-metal or initramfs environments do not attach a persistent block device.
        // Kernel and initrd injection is handled by the boot module.
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

            let disk_xml = format!(
                r#"<disk type='file' device='disk'>
      <driver name='qemu' type='raw'/>
      <source file='{}'/>
      <target dev='vda' bus='virtio'/>
    </disk>"#,
                ic.disk_image
            );

            c.devices_xml.push(disk_xml);
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

            // Write state file so lib.rs provision_lvm_root() knows what to create
            let disk_state_file = fc.crundir.join("disk");
            fs::write(disk_state_file, format!("{} {}\n", lv, size_mb))?;

            // Generate Libvirt XML for a block device (LVM)
            let disk_xml = format!(
                r#"<disk type='block' device='disk'>
      <driver name='qemu' type='raw'/>
      <source dev='{}'/>
      <target dev='vda' bus='virtio'/>
    </disk>"#,
                lv
            );

            c.devices_xml.push(disk_xml);
            Ok(())
        }
        other => Err(format!("unknown disk_type \"{}\" in boot/config.json (expected \"\", \"file\" or \"lvm\")", other).into())
    }
}

// Create the per-container logical volume and populate it with a clone
// of the container rootfs, so the docker image becomes the guest's
// persistent root filesystem: lvcreate -> mkfs.ext4 -> mount -> copy ->
// umount. On any failure after lvcreate the LV is removed again so a
// failed create leaves no leaked volume behind.
pub fn provision_lvm_root(
    lv: &str,
    size_mb: &str,
    rootfs: &Path,
    crundir: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut comps = lv.rsplitn(3, '/');
    let lvname = comps.next().ok_or("malformed lv path")?;
    let vg = comps.next().ok_or("malformed lv path")?;

    run_command(
        Command::new("lvcreate")
            .arg("-y")
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
