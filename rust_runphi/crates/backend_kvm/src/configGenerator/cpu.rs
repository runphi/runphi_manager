use std::error::Error;
use std::path::Path;
use std::env;

use crate::configGenerator;
use f2b;


pub fn cpuconf(
    fc: &f2b::FrontendConfig,
    ic: &f2b::ImageConfig,
    c: &mut configGenerator::BackendConfig,
) -> Result<(), Box<dyn Error>> {

    let has_kvm = Path::new("/dev/kvm").exists();

     //TODO(lorenzo): Qua si deve capire se va bene anche per arm
    #[cfg(target_arch = "aarch64")]
    {
        let host_arch = env::consts::ARCH;
        c.add_arg("-M", "virt,gic-version=max");

        if has_kvm && host_arch == target_arch {
            c.add_flag("-enable-kvm");
            c.add_arg("-cpu", "host");
        } else {
            c.add_arg("-cpu", "max");
        }
        
    }
    #[cfg(target_arch = "x86_64")]
    {
        c.add_arg("-M", "q35");

        if has_kvm {
            c.add_arg("-cpu", "host");
            c.add_flag("-enable-kvm");
        } else {
            c.add_arg("-cpu", "qemu64");
        }
    }

    // NOTE(lorenzo): vCPU pinning must be done in createguest, because we need to communicate with QEMU to obtain the TIDs to pin to pCPUs

    let period = fc.jsonconfig["linux"]["resources"]["cpu"]["period"]
        .as_f64()
        .unwrap_or(0.0);
    let quota = fc.jsonconfig["linux"]["resources"]["cpu"]["quota"]
        .as_f64()
        .unwrap_or(0.0);
    
    let oci_cpus = if period > 0.0 && quota > 0.0 {
        (quota/period).ceil() as u32
    } else {
        0
    };

    // NOTE(lorenzo): User specified the number of vcpus to allocate
    let allocated_vcpus = if ic.vcpus > 0 {
        ic.vcpus
    } else if !ic.vcpu_pinning.is_empty() { // NOTE(lorenzo): The user didnt specify vcpus, but specified a vCPU pinning
        ic.vcpu_pinning.len() as u32
    } else if oci_cpus > 0 {        // NOTE(lorenzo): Fallback if nothing got specified
        oci_cpus
    } else {
        1
    };  

    if oci_cpus > 0 && allocated_vcpus > oci_cpus {
        logging::log_message(logging::Level::Info, format!("runPHI is allocating {} vCPUs, 
                            but the container has a limit of {:.1} CPUs (quota: {})", 
                            allocated_vcpus, (quota/period), quota).as_str());
    }

    c.add_arg("-smp", format!("{}", allocated_vcpus));

    Ok(())
}
