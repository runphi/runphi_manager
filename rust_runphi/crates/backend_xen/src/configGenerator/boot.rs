//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//*********************************************


use f2b;
use crate::configGenerator;

//const WORKPATH: &str = "/usr/share/runPHI";
//const RUNDIR: &str = "/run/runPHI";

//TODO: replace multiple panics of this function with something more informative
pub fn bootconf(
    _fc: &f2b::FrontendConfig, 
    c: &mut configGenerator::BackendConfig, 
    ic: &mut f2b::ImageConfig) {

    let _nonrootdefaultpath = "/root/runPHI/demo_containers";
    let _xen_path = "/etc/xen";

    // Here if a Kernel and a ramdisk are provided by client a linux-non-root-cell has to be started
    // a reference to them is stored in crundir to be used when create is called
    // if no kernel and ramdisk are provided, default are used
    if ic.inmate.is_empty(){
        //ic.kernel = format!("{}/linux/Image", nonrootdefaultpath).to_string();
        ic.inmate = "/root/vmlinuz".to_string().to_string();
    }
    
    if ic.ramdisk.is_empty(){
        //for x68_64, we use the ramdisk provided by the image
        //ic.ramdisk = format!("/root/initrd.gz").to_string(); //Commented becauses raises an error if the file does not exist
    }

    // The kernel line is common to every Xen guest: for a bare-metal inmate it
    // is the binary itself, for a Linux domU it is the kernel Image. ic.inmate
    // is already resolved against the rootfs mountpoint by ImageConfig.
    c.conf.push_str(&format!("kernel = \"{}\" \n", ic.inmate));

    // A Linux domU additionally needs an initramfs to act as its root
    // filesystem, a kernel command line, and (on x86) an explicit PVH boot
    // type. Bare-metal guests (os_var != "linux") take none of this, so their
    // generated config is byte-for-byte identical to before.
    let is_linux = ic.os_var.eq_ignore_ascii_case("linux");
    if is_linux {
        // On x86 a directly-booted Linux kernel runs as a PVH guest. On ARM
        // Xen has a single guest type, so no `type` line is emitted there.
        #[cfg(target_arch = "x86_64")]
        c.conf.push_str("type = \"pvh\" \n");

        // Root filesystem comes from the initramfs shipped in the image. The
        // path is already resolved against the rootfs mountpoint by ImageConfig.
        if !ic.ramdisk.is_empty() {
            c.conf.push_str(&format!("ramdisk = \"{}\" \n", ic.ramdisk));
        }

        // Minimal kernel command line: route the console to the Xen PV console
        // so `xl console` shows the boot. With a disk-backed root ("file" or
        // "lvm" in disk.rs, both attached as xvda) point root= at it; without
        // one the root is the unpacked initramfs and no root= is required.
        if matches!(ic.disk_type.as_str(), "file" | "lvm") {
            c.conf.push_str("extra = \"console=hvc0 root=/dev/xvda\" \n");
        } else {
            c.conf.push_str("extra = \"console=hvc0\" \n");
        }
    }

    //idk what to do with cpio

    //if we use PV Guest , we don't need emulaiton
    //if ic.dtb.is_empty() {
    //    ic.dtb = format!("{}/configs/arm64/dts/inmate-qemu-arm64.dtb", xenpath).to_string();
    //}
}

#[cfg(test)]
mod tests {
    use super::*;

    // All ImageConfig fields are #[serde(default)], so an empty JSON object
    // yields a fully-defaulted instance we can tweak per case.
    fn image_config(os_var: &str, inmate: &str, ramdisk: &str) -> f2b::ImageConfig {
        let mut ic: f2b::ImageConfig = serde_json::from_str("{}").unwrap();
        ic.os_var = os_var.to_string();
        ic.inmate = inmate.to_string();
        ic.ramdisk = ramdisk.to_string();
        ic
    }

    fn run_bootconf(ic: &mut f2b::ImageConfig) -> String {
        let fc = f2b::FrontendConfig::new();
        let mut c = configGenerator::BackendConfig::new();
        bootconf(&fc, &mut c, ic);
        c.conf
    }

    // Bare-metal (os_var != "linux") must emit exactly the kernel line and
    // nothing else: this is the regression guard for existing inmate guests.
    #[test]
    fn baremetal_emits_only_kernel_line() {
        let mut ic = image_config("zephyr", "/mnt/rootfs/boot/hello.bin", "");
        let conf = run_bootconf(&mut ic);
        assert_eq!(conf, "kernel = \"/mnt/rootfs/boot/hello.bin\" \n");
    }

    // A Linux domU adds a ramdisk and a kernel command line on top of kernel.
    #[test]
    fn linux_emits_kernel_ramdisk_and_cmdline() {
        let mut ic = image_config(
            "linux",
            "/mnt/rootfs/boot/Image",
            "/mnt/rootfs/boot/rootfs.cpio.gz",
        );
        let conf = run_bootconf(&mut ic);
        assert!(conf.contains("kernel = \"/mnt/rootfs/boot/Image\" \n"));
        assert!(conf.contains("ramdisk = \"/mnt/rootfs/boot/rootfs.cpio.gz\" \n"));
        assert!(conf.contains("extra = \"console=hvc0\" \n"));
        // os_var matching is case-insensitive.
        let mut ic_upper = image_config("Linux", "/k", "/r");
        assert!(run_bootconf(&mut ic_upper).contains("ramdisk = \"/r\" \n"));
    }

    // With a disk-backed root (file or lvm) the kernel command line must
    // point root= at xvda; without one it must not mention root= at all.
    #[test]
    fn linux_disk_root_sets_root_cmdline() {
        for disk_type in ["file", "lvm"] {
            let mut ic = image_config("linux", "/k", "");
            ic.disk_type = disk_type.to_string();
            let conf = run_bootconf(&mut ic);
            assert!(
                conf.contains("extra = \"console=hvc0 root=/dev/xvda\" \n"),
                "missing root= for disk_type={}",
                disk_type
            );
        }
        let mut ic = image_config("linux", "/k", "/r");
        assert!(!run_bootconf(&mut ic).contains("root="));
    }

    // On x86 a Linux guest is booted as PVH; bare-metal never gets a type line.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn x86_linux_emits_pvh_type_baremetal_does_not() {
        let mut linux = image_config("linux", "/k", "/r");
        assert!(run_bootconf(&mut linux).contains("type = \"pvh\" \n"));
        let mut bare = image_config("zephyr", "/k", "");
        assert!(!run_bootconf(&mut bare).contains("type ="));
    }
}
