use std::error::Error;

use crate::configGenerator;
use f2b;

pub fn bootconf(
    ic: &f2b::ImageConfig,
    c: &mut configGenerator::BackendConfig,
    is_linux: &bool
) -> Result<(), Box<dyn Error>> {

    if *is_linux {
        if !ic.inmate.is_empty() {
            c.add_arg("-kernel", &ic.inmate);
        }
        if !ic.ramdisk.is_empty() {
            c.add_arg("-initrd", &ic.ramdisk);
        }
        if !ic.dtb.is_empty() {
            c.add_arg("-dtb", &ic.dtb);
        }
        c.add_arg("-append", "console=ttyS0,115200");
    } else {
        c.add_arg("-kernel", &ic.inmate);
    }
    
    Ok(())
}