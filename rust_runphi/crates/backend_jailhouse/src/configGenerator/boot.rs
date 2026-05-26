//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//*********************************************

use std::error::Error;
use std::fs::{self};
use std::process::{self};

use f2b;

//const WORKPATH: &str = "/usr/share/runPHI";
//const RUNDIR: &str = "/run/runPHI";

pub fn bootconfbackend(
    fc: &f2b::FrontendConfig,
    ic: &mut f2b::ImageConfig,
) -> Result<(), Box<dyn Error>> {
    let nonrootdefaultpath = "/root/runPHI/demo_containers";
    let jailhousepath = "/root/jailhouse";

    // Here if a Kernel and a ramdisk are provided by client a linux-non-root-cell has to be started
    // a reference to them is stored in crundir to be used when create is called
    // if no kernel and ramdisk are provided, default are used
    if ic.kernel.is_empty() {
        ic.kernel = format!("{}/linux/Image", nonrootdefaultpath).to_string();
    }

    if !ic.cpio.is_empty() {
        //TODO: does this work??? test
        // Create a .cpio filesystem from rootfs and save it in rootfs/cpio.cpio
        let status = process::Command::new("cpio")
            .arg("-ov")
            .arg(">")
            .arg(format!("{}/cpio.cpio", fc.mountpoint))
            .status()?;
        if !status.success() {
            return Err(format!("cpio failed (exit {})", status.code().unwrap_or(-1)).into());
        }

        let cpio_content = fs::read_to_string(format!("{}/cpio.cpio", fc.mountpoint))?;
        ic.cpio = cpio_content;
    } else {
        ic.cpio = format!("{}/linux/rootfs.cpio.gz", nonrootdefaultpath).to_string();
    }

    if ic.dtb.is_empty() {
        ic.dtb = format!("{}/configs/arm64/dts/inmate-qemu-arm64.dtb", jailhousepath).to_string();
    }

    Ok(())
}
