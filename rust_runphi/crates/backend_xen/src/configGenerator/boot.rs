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
    c: &mut configGenerator::Backendconfig, 
    ic: &mut f2b::ImageConfig) {

    let _nonrootdefaultpath = "/root/runPHI/demo_containers";
    let _xen_path = "/etc/xen";

    // Here if a Kernel and a ramdisk are provided by client a linux-non-root-cell has to be started
    // a reference to them is stored in crundir to be used when create is called
    // if no kernel and ramdisk are provided, default are used
    if ic.inmate.is_empty(){
        //ic.kernel = format!("{}/linux/Image", nonrootdefaultpath).to_string();
        ic.inmate = format!("/root/vmlinuz").to_string();
    }
    
    if ic.ramdisk.is_empty(){
        ic.ramdisk = format!("/root/initrd.gz").to_string();
    }

    c.conf.
        push_str(&format!("\n\nkernel = \"{}\" \n\nramdisk = \"{}\" \n\n", ic.inmate, ic.ramdisk));



    //idk what to do with cpio

    //if we use PV Guest , we don't need emulaiton
    //if ic.dtb.is_empty() {
    //    ic.dtb = format!("{}/configs/arm64/dts/inmate-qemu-arm64.dtb", xenpath).to_string();
    //}
}
