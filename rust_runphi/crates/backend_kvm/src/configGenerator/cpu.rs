use std::error::Error;
use std::path::Path;

use crate::configGenerator;
use f2b;

/// Machine model, platform features, CPU model and domain type for a guest
/// of the host architecture `arch`, with KVM (`has_kvm`) or TCG emulation.
pub fn machine_conf(
    arch: &str,
    has_kvm: bool,
    c: &mut configGenerator::BackendConfig,
) -> Result<(), Box<dyn Error>> {
    match arch {
        "aarch64" => {
            c.os_arch = "aarch64".to_string();
            c.os_machine = "virt".to_string();
            if has_kvm {
                // KVM can only give the guest the GIC version of the host
                // (GICv2 on Zynq UltraScale+ / Kria, GICv3 on servers), so let
                // QEMU take it from the host instead of hardcoding one.
                c.features_xml = "<gic version='host'/>".to_string();
                c.domain_type = "kvm".to_string();
                c.cpu_xml = "<cpu mode='host-passthrough' check='none'/>".to_string();
            } else {
                // "host" needs KVM; the emulated virt board defaults to GICv3.
                c.features_xml = "<gic version='3'/>".to_string();
                c.domain_type = "qemu".to_string();
                c.cpu_xml =
                    "<cpu mode='custom' match='exact'><model fallback='forbid'>max</model></cpu>"
                        .to_string();
            }
        }
        "x86_64" => {
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
        other => {
            return Err(format!(
                "unsupported host architecture '{}' (expected aarch64 or x86_64)",
                other
            )
            .into())
        }
    }
    Ok(())
}

pub fn cpuconf(
    fc: &f2b::FrontendConfig,
    ic: &f2b::ImageConfig,
    c: &mut configGenerator::BackendConfig,
) -> Result<(), Box<dyn Error>> {
    let has_kvm = Path::new("/dev/kvm").exists();
    machine_conf(std::env::consts::ARCH, has_kvm, c)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn conf(arch: &str, has_kvm: bool) -> configGenerator::BackendConfig {
        let mut c = configGenerator::BackendConfig::new();
        machine_conf(arch, has_kvm, &mut c).unwrap();
        c
    }

    #[test]
    fn test_machine_conf_aarch64_kvm_uses_host_gic() {
        let c = conf("aarch64", true);
        assert_eq!(c.os_arch, "aarch64");
        assert_eq!(c.os_machine, "virt");
        assert_eq!(c.domain_type, "kvm");
        assert_eq!(c.features_xml, "<gic version='host'/>");
        assert!(c.cpu_xml.contains("host-passthrough"));
    }

    #[test]
    fn test_machine_conf_aarch64_tcg() {
        let c = conf("aarch64", false);
        assert_eq!(c.domain_type, "qemu");
        assert_eq!(c.features_xml, "<gic version='3'/>");
        assert!(c.cpu_xml.contains("<model fallback='forbid'>max</model>"));
    }

    #[test]
    fn test_machine_conf_x86_64() {
        let c = conf("x86_64", true);
        assert_eq!(c.os_machine, "q35");
        assert_eq!(c.domain_type, "kvm");
        assert!(c.features_xml.contains("<acpi/>"));
        assert!(!c.features_xml.contains("gic"));

        assert_eq!(conf("x86_64", false).domain_type, "qemu");
    }

    #[test]
    fn test_machine_conf_unsupported_arch() {
        let mut c = configGenerator::BackendConfig::new();
        assert!(machine_conf("riscv64", true, &mut c).is_err());
    }
}
