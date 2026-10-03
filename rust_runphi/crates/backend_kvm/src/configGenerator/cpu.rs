use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
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

/// A set of CPUs as a cpulist ("0-2,5"), the format of the kernel, of
/// libvirt's cpuset attributes and of OCI's linux.resources.cpu.cpus.
pub fn format_cpulist(cpus: &BTreeSet<usize>) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut iter = cpus.iter().copied().peekable();
    while let Some(first) = iter.next() {
        let mut last = first;
        while iter.peek() == Some(&(last + 1)) {
            last = iter.next().unwrap();
        }
        out.push(if first == last {
            first.to_string()
        } else {
            format!("{}-{}", first, last)
        });
    }
    out.join(",")
}

fn cpulist(s: &str) -> BTreeSet<usize> {
    crate::irq::parse_cpulist(s).into_iter().collect()
}

/// The container's cpuset (`docker run --cpuset-cpus`), if it has one.
pub fn container_cpus(fc: &f2b::FrontendConfig) -> Option<BTreeSet<usize>> {
    fc.jsonconfig["linux"]["resources"]["cpu"]["cpus"]
        .as_str()
        .map(cpulist)
        .filter(|cpus| !cpus.is_empty())
}

/// Where QEMU's emulator threads (everything but the vCPUs: main loop, I/O
/// and monitor threads) should run, or None to leave them with the vCPUs.
///
/// A pinned vCPU runs at SCHED_FIFO 99. An emulator thread sharing its CPU
/// only runs when that vCPU sleeps, and not at all when the vCPU keeps the
/// CPU busy, as KVM does on arm64 after a guest's PSCI SYSTEM_OFF: QEMU then
/// never handles the shutdown nor answers libvirt, and the container cannot
/// be stopped. So, unless `explicit` (emulator_pinning) says otherwise, the
/// emulator threads go to the first non-empty of:
/// 1. the container's CPUs (`allowed`) that no vCPU is pinned to, not isolated;
/// 2. the same, isolated ones included;
/// 3. the host's housekeeping CPUs: online, not pinned, not isolated;
/// 4. any online CPU no vCPU is pinned to.
/// Without pinned vCPUs nothing changes. `explicit` = [] disables this.
pub fn choose_emulator_cpus(
    explicit: Option<&[usize]>,
    pinned: &BTreeSet<usize>,
    allowed: &BTreeSet<usize>,
    online: &BTreeSet<usize>,
    isolated: &BTreeSet<usize>,
) -> Option<BTreeSet<usize>> {
    if let Some(list) = explicit {
        return if list.is_empty() {
            None
        } else {
            Some(list.iter().copied().collect())
        };
    }
    if pinned.is_empty() {
        return None;
    }
    let free: BTreeSet<usize> = allowed.difference(pinned).copied().collect();
    let free_hk: BTreeSet<usize> = free.difference(isolated).copied().collect();
    let host: BTreeSet<usize> = online.difference(pinned).copied().collect();
    let host_hk: BTreeSet<usize> = host.difference(isolated).copied().collect();
    [free_hk, free, host_hk, host].into_iter().find(|s| !s.is_empty())
}

/// emulator_pinning resolved for this host and container (see
/// choose_emulator_cpus). config_generate stores the result back into the
/// ImageConfig, so that the domain XML, the cgroup and the thread pinning in
/// createguest all use the same CPUs.
pub fn emulator_cpus(fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Option<BTreeSet<usize>> {
    let pinned: BTreeSet<usize> = ic.vcpu_pinning.iter().map(|p| p.pcpu).collect();
    let online = fs::read_to_string("/sys/devices/system/cpu/online")
        .map(|s| cpulist(&s))
        .unwrap_or_default();
    let allowed = container_cpus(fc).unwrap_or_else(|| online.clone());
    let isolated: BTreeSet<usize> = crate::irq::get_isolated_cpus(ic).into_iter().collect();
    let cpus = choose_emulator_cpus(ic.emulator_pinning.as_deref(), &pinned, &allowed, &online, &isolated);
    if let Some(cpus) = &cpus {
        if !cpus.is_disjoint(&pinned) {
            logging::log_message(
                logging::Level::Warn,
                &format!(
                    "emulator_pinning {} shares CPUs with pinned vCPUs ({}): QEMU's main loop can starve behind a SCHED_FIFO vCPU",
                    format_cpulist(cpus),
                    format_cpulist(&pinned)
                ),
            );
        }
    }
    cpus
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

    // Set the defined vCPU pinning and apply SCHED_FIFO priority to vCPU,
    // and keep QEMU's other threads off those CPUs (emulator_pinning, as
    // resolved by config_generate).
    let emulator: Option<BTreeSet<usize>> = ic
        .emulator_pinning
        .as_ref()
        .filter(|cpus| !cpus.is_empty())
        .map(|cpus| cpus.iter().copied().collect());
    if !ic.vcpu_pinning.is_empty() || emulator.is_some() {
        let mut cputune = String::from("<cputune>\n");
        for pin in &ic.vcpu_pinning {
            cputune.push_str(&format!(
                    "    <vcpupin vcpu='{}' cpuset='{}'/>\n",
                    pin.vcpu, pin.pcpu
            ));
        }

        // Give every pinned vCPU real-time host scheduling priority.
        if !ic.vcpu_pinning.is_empty() {
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
        }

        if let Some(cpus) = &emulator {
            cputune.push_str(&format!("    <emulatorpin cpuset='{}'/>\n", format_cpulist(cpus)));
        }

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

    fn set(cpus: &[usize]) -> BTreeSet<usize> {
        cpus.iter().copied().collect()
    }

    #[test]
    fn test_format_cpulist() {
        assert_eq!(format_cpulist(&set(&[])), "");
        assert_eq!(format_cpulist(&set(&[3])), "3");
        assert_eq!(format_cpulist(&set(&[0, 1, 2])), "0-2");
        assert_eq!(format_cpulist(&set(&[0, 2, 3, 5])), "0,2-3,5");
    }

    #[test]
    fn test_choose_emulator_cpus() {
        let online = set(&[0, 1, 2, 3]);
        let none = BTreeSet::new();
        // No pinned vCPUs: nothing to protect, unless asked explicitly.
        assert_eq!(choose_emulator_cpus(None, &none, &online, &online, &none), None);
        assert_eq!(
            choose_emulator_cpus(Some(&[1]), &none, &online, &online, &none),
            Some(set(&[1]))
        );
        // Explicitly disabled.
        assert_eq!(choose_emulator_cpus(Some(&[]), &set(&[3]), &online, &online, &none), None);
        // --cpuset-cpus 3, vCPU on 3, CPU 3 isolated: the host's housekeeping CPUs.
        assert_eq!(
            choose_emulator_cpus(None, &set(&[3]), &set(&[3]), &online, &set(&[3])),
            Some(set(&[0, 1, 2]))
        );
        // --cpuset-cpus 2,3, vCPUs on 2 and 3, CPU 3 isolated: 0 and 1.
        assert_eq!(
            choose_emulator_cpus(None, &set(&[2, 3]), &set(&[2, 3]), &online, &set(&[3])),
            Some(set(&[0, 1]))
        );
        // --cpuset-cpus 1-3, vCPU on 3, 2-3 isolated: the container's own CPU 1.
        assert_eq!(
            choose_emulator_cpus(None, &set(&[3]), &set(&[1, 2, 3]), &online, &set(&[2, 3])),
            Some(set(&[1]))
        );
        // --cpuset-cpus 2,3, vCPU on 3, both isolated: the container's CPU 2.
        assert_eq!(
            choose_emulator_cpus(None, &set(&[3]), &set(&[2, 3]), &online, &set(&[2, 3])),
            Some(set(&[2]))
        );
        // Every host CPU isolated: any CPU without a pinned vCPU.
        assert_eq!(
            choose_emulator_cpus(None, &set(&[3]), &set(&[3]), &online, &online),
            Some(set(&[0, 1, 2]))
        );
        // Every CPU has a pinned vCPU: nowhere else to go.
        assert_eq!(choose_emulator_cpus(None, &online, &online, &online, &none), None);
    }

    fn cputune(json: serde_json::Value) -> String {
        let ic: f2b::ImageConfig = serde_json::from_value(json).unwrap();
        let mut c = configGenerator::BackendConfig::new();
        cpuconf(&f2b::FrontendConfig::new(), &ic, &mut c).unwrap();
        c.cputune_xml
    }

    #[test]
    fn test_cputune_emulatorpin() {
        let xml = cputune(serde_json::json!({
            "vcpu_pinning": [{"vcpu": 0, "pcpu": 3}],
            "emulator_pinning": [0, 1, 2]
        }));
        assert!(xml.contains("<vcpupin vcpu='0' cpuset='3'/>"));
        assert!(xml.contains("<vcpusched vcpus='0' scheduler='fifo' priority='99'/>"));
        assert!(xml.contains("<emulatorpin cpuset='0-2'/>"));
        // The element libvirt sees, well-formed.
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let pin = doc.descendants().find(|n| n.has_tag_name("emulatorpin")).unwrap();
        assert_eq!(pin.attribute("cpuset"), Some("0-2"));

        // Resolved to "none": no emulatorpin; no pinning at all: no cputune.
        let xml = cputune(serde_json::json!({
            "vcpu_pinning": [{"vcpu": 0, "pcpu": 3}],
            "emulator_pinning": []
        }));
        assert!(!xml.contains("emulatorpin"));
        assert_eq!(cputune(serde_json::json!({})), "");
        // The libvirt-style alias.
        let ic: f2b::ImageConfig = serde_json::from_value(serde_json::json!({"emulatorpin": [1]})).unwrap();
        assert_eq!(ic.emulator_pinning, Some(vec![1]));
    }

    #[test]
    fn test_machine_conf_unsupported_arch() {
        let mut c = configGenerator::BackendConfig::new();
        assert!(machine_conf("riscv64", true, &mut c).is_err());
    }
}
