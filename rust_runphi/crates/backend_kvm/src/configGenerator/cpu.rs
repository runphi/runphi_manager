use std::error::Error;
use std::path::Path;
#[cfg(target_arch = "aarch64")]
use std::env;

use crate::configGenerator;
use f2b;

pub fn cpuconf(
    fc: &f2b::FrontendConfig,
    ic: &f2b::ImageConfig,
    c: &mut configGenerator::BackendConfig,
) -> Result<(), Box<dyn Error>> {
    let has_kvm = Path::new("/dev/kvm").exists();

    #[cfg(target_arch = "aarch64")]
    {
        let host_arch = env::consts::ARCH;
        c.os_arch = "aarch64".to_string();
        c.os_machine = "virt".to_string();
        c.features_xml = "<gic version='3'/>".to_string();

        if has_kvm && host_arch == "aarch64" {
            c.domain_type = "kvm".to_string();
            c.cpu_xml = "<cpu mode='host-passthrough' check='none'/>".to_string();
        } else {
            c.domain_type = "qemu".to_string();
            c.cpu_xml = "<cpu mode='custom' match='exact'><model fallback='forbid'>max</model></cpu>".to_string();
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        c.os_arch = "x86_64".to_string();
        c.os_machine = "q35".to_string();
        c.features_xml = "<acpi/>\n    <apic/>".to_string();

        if has_kvm {
            c.domain_type = "kvm".to_string();
            c.cpu_xml = "<cpu mode='host-passthrough' check='none'/>".to_string();
        } else {
            c.domain_type = "qemu".to_string();
            c.cpu_xml = "<cpu mode='custom' match='exact'><model fallback='forbid'>qemu64</model></cpu>".to_string();
        }
    }

    let period = fc.jsonconfig["linux"]["resources"]["cpu"]["period"]
        .as_f64()
        .unwrap_or(0.0);
    let quota = fc.jsonconfig["linux"]["resources"]["cpu"]["quota"]
        .as_f64()
        .unwrap_or(0.0);

    let oci_cpus = if period > 0.0 && quota > 0.0 {
        (quota / period).ceil() as u32
    } else {
        0
    };

    let allocated_vcpus = if ic.vcpus > 0 {
        ic.vcpus
    } else if !ic.vcpu_pinning.is_empty() {
        ic.vcpu_pinning.len() as u32
    } else if oci_cpus > 0 {
        oci_cpus
    } else {
        1
    };

    if oci_cpus > 0 && allocated_vcpus > oci_cpus {
        logging::log_message(
            logging::Level::Info,
            format!(
                "runPHI is allocating {} vCPUs, but the container has a limit of {:.1} CPUs (quota: {})",
                allocated_vcpus,
                (quota / period),
                quota
            )
            .as_str(),
        );
    }


    c.vcpus = allocated_vcpus;

    // Set the defined vCPU pinning and apply SCHED_FIFO priority to vCPU
    if !ic.vcpu_pinning.is_empty() {
        let mut cputune = String::from("<cputune>\n");
        for pin in &ic.vcpu_pinning {
            cputune.push_str(&format!(
                    "    <vcpupin vcpu='{}' cpuset='{}'/>\n",
                    pin.vcpu, pin.pcpu
            ));
        }

        // Give every pinned vCPU real-time host scheduling priority.
        let vcpu_list = ic
            .vcpu_pinning
            .iter()
            .map(|p| p.vcpu.to_string())
            .collect::<Vec<_>>()
            .join(",");
        cputune.push_str(&format!(
                "    <vcpusched vcpus='{}' scheduler='fifo' priority='99'/>\n",
                vcpu_list
        ));

        cputune.push_str("  </cputune>");
        c.cputune_xml = cputune;
    }

    // Validate IRQ steering target CPUs and warn if any are isolated
    if let Some(cpus) = crate::irq::get_steer_irqs(fc, ic) {
        crate::irq::warn_if_isolated(&cpus, ic);
    }

    Ok(())
}
