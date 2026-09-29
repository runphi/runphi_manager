use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::Path;

use f2b;

/// Parse a Linux cpulist string (e.g. "0", "1,2", "1-3,5") into a set of CPU IDs.
pub fn parse_cpulist(s: &str) -> HashSet<usize> {
    let mut set = HashSet::new();
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return set;
    }

    for part in trimmed.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some((start_str, end_str)) = part.split_once('-') {
            if let (Ok(start), Ok(end)) = (
                start_str.trim().parse::<usize>(),
                end_str.trim().parse::<usize>(),
            ) {
                for cpu in start..=end {
                    set.insert(cpu);
                }
            }
        } else if let Ok(cpu) = part.parse::<usize>() {
            set.insert(cpu);
        }
    }

    set
}

/// Collect all isolated CPUs identified by:
/// 1. Host Linux sysfs (/sys/devices/system/cpu/isolated)
/// 2. Host Linux sysfs (/sys/devices/system/cpu/nohz_full)
/// 3. Container ImageConfig isolcpu field
/// 4. Container ImageConfig nohz_full field
pub fn get_isolated_cpus(ic: &f2b::ImageConfig) -> HashSet<usize> {
    let mut isolated = HashSet::new();

    // 1. Host isolated CPUs
    if let Ok(content) = fs::read_to_string("/sys/devices/system/cpu/isolated") {
        isolated.extend(parse_cpulist(&content));
    }

    // 2. Host full dynticks (nohz_full) CPUs
    if let Ok(content) = fs::read_to_string("/sys/devices/system/cpu/nohz_full") {
        isolated.extend(parse_cpulist(&content));
    }

    // 3. Container-level isolcpu
    if !ic.isolcpu.is_empty() {
        isolated.extend(parse_cpulist(&ic.isolcpu));
    }

    // 4. Container-level nohz_full
    if !ic.nohz_full.is_empty() {
        isolated.extend(parse_cpulist(&ic.nohz_full));
    }

    isolated
}

/// Resolve the list of target CPUs for IRQ steering from ImageConfig or FrontendConfig fallback.
pub fn get_steer_irqs(fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Option<Vec<usize>> {
    if let Some(ref cpus) = ic.steer_irq {
        if !cpus.is_empty() {
            return Some(cpus.clone());
        }
    }

    // Fallback: check OCI bundle config.json
    if let Some(arr) = fc.jsonconfig.get("steer_irq").and_then(|v| v.as_array()) {
        let cpus: Vec<usize> = arr
            .iter()
            .filter_map(|v| v.as_u64().map(|n| n as usize))
            .collect();
        if !cpus.is_empty() {
            return Some(cpus);
        }
    }

    None
}

/// Warn the user through the logging library if any CPU in the target steering list is isolated.
pub fn warn_if_isolated(steer_cpus: &[usize], ic: &f2b::ImageConfig) {
    let isolated = get_isolated_cpus(ic);

    for &cpu in steer_cpus {
        if isolated.contains(&cpu) {
            logging::log_message(
                logging::Level::Warn,
                &format!(
                    "CPU {} is isolated, but specified in steer_irq! Redirecting IRQs to an isolated CPU may compromise real-time performance.",
                    cpu
                ),
            );
        }
    }
}

/// Apply IRQ steering across host interrupts in /proc/irq/<irq>/smp_affinity_list.
/// Pre-existing affinities are saved to crundir/saved_irq_affinities.json for cleanup.
pub fn apply_irq_steering(
    crundir: &Path,
    ic: &f2b::ImageConfig,
    steer_cpus: &[usize],
) -> Result<(), Box<dyn Error>> {
    if steer_cpus.is_empty() {
        return Ok(());
    }

    // 1. Warn if any target CPU is isolated
    warn_if_isolated(steer_cpus, ic);

    let irq_dir = Path::new("/proc/irq");
    if !irq_dir.exists() {
        logging::log_message(
            logging::Level::Warn,
            "/proc/irq does not exist; skipping IRQ steering.",
        );
        return Ok(());
    }

    let cpulist_str = steer_cpus
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let mut saved_affinities: HashMap<u32, String> = HashMap::new();
    let mut modified_count = 0;

    let entries = fs::read_dir(irq_dir)?;
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();

        // Check if directory name is numeric (an IRQ number)
        if let Ok(irq_num) = name_str.parse::<u32>() {
            let aff_path = entry.path().join("smp_affinity_list");
            if aff_path.exists() {
                // Save original affinity if not already saved
                if let Ok(original) = fs::read_to_string(&aff_path) {
                    saved_affinities.insert(irq_num, original.trim().to_string());
                }

                // Write new target affinity
                match fs::write(&aff_path, &cpulist_str) {
                    Ok(_) => {
                        modified_count += 1;
                    }
                    Err(e) => {
                        // Certain IRQs (timer IRQ 0, arch-specific IPIs) cannot be moved by design
                        logging::log_message(
                            logging::Level::Trace,
                            &format!(
                                "Could not set affinity for IRQ {} to {}: {}",
                                irq_num, cpulist_str, e
                            ),
                        );
                    }
                }
            }
        }
    }

    // Write saved affinities to crundir
    let state_file = crundir.join("saved_irq_affinities.json");
    if let Ok(json) = serde_json::to_string_pretty(&saved_affinities) {
        if let Err(e) = fs::write(&state_file, json) {
            logging::log_message(
                logging::Level::Warn,
                &format!("Failed to save original IRQ affinities to {}: {}", state_file.display(), e),
            );
        }
    }

    logging::log_message(
        logging::Level::Info,
        &format!("Steered {} IRQs to CPUs [{}]", modified_count, cpulist_str),
    );

    Ok(())
}

/// Restore original IRQ affinities saved in crundir/saved_irq_affinities.json.
pub fn restore_irq_steering(crundir: &Path) -> Result<(), Box<dyn Error>> {
    let state_file = crundir.join("saved_irq_affinities.json");
    if !state_file.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&state_file)?;
    let saved: HashMap<u32, String> = serde_json::from_str(&content)?;
    let mut restored_count = 0;

    for (irq_num, orig_aff) in &saved {
        let aff_path = Path::new("/proc/irq")
            .join(irq_num.to_string())
            .join("smp_affinity_list");

        if aff_path.exists() {
            if fs::write(&aff_path, orig_aff).is_ok() {
                restored_count += 1;
            }
        }
    }

    let _ = fs::remove_file(&state_file);

    logging::log_message(
        logging::Level::Info,
        &format!("Restored SMP affinity for {} IRQs", restored_count),
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cpulist_empty() {
        assert!(parse_cpulist("").is_empty());
        assert!(parse_cpulist("   ").is_empty());
    }

    #[test]
    fn test_parse_cpulist_single() {
        let set = parse_cpulist("3");
        assert_eq!(set.len(), 1);
        assert!(set.contains(&3));
    }

    #[test]
    fn test_parse_cpulist_list() {
        let set = parse_cpulist("0,2,4");
        assert_eq!(set.len(), 3);
        assert!(set.contains(&0));
        assert!(set.contains(&2));
        assert!(set.contains(&4));
    }

    #[test]
    fn test_parse_cpulist_range() {
        let set = parse_cpulist("1-3");
        assert_eq!(set.len(), 3);
        assert!(set.contains(&1));
        assert!(set.contains(&2));
        assert!(set.contains(&3));
    }

    #[test]
    fn test_parse_cpulist_mixed() {
        let set = parse_cpulist(" 0, 2-4, 7 ");
        assert_eq!(set.len(), 5);
        assert!(set.contains(&0));
        assert!(set.contains(&2));
        assert!(set.contains(&3));
        assert!(set.contains(&4));
        assert!(set.contains(&7));
    }

    #[test]
    fn test_detect_isolated_cpus_from_image_config() {
        let ic = f2b::ImageConfig {
            cpio: String::new(),
            os_var: String::new(),
            kernel: String::new(),
            ramdisk: String::new(),
            inmate: String::new(),
            dtb: String::new(),
            initrd: String::new(),
            netconf: String::new(),
            disk_type: String::new(),
            disk_image: String::new(),
            disk_size: 0,
            starting_vaddress: String::new(),
            memory: 0,
            net: String::new(),
            rpu_req: false,
            vcpus: 1,
            vcpu_pinning: Vec::new(),
            isolcpu: "2,3".to_string(),
            nohz_full: "3".to_string(),
            steer_irq: Some(vec![0, 2]),
        };

        let isolated = get_isolated_cpus(&ic);
        assert!(isolated.contains(&2));
        assert!(isolated.contains(&3));
        assert!(!isolated.contains(&0));
        assert!(!isolated.contains(&1));
    }

    #[test]
    fn test_steer_irq_deserialization() {
        let json_data = r#"{
            "vcpus": 2,
            "steer_irq": [0, 1]
        }"#;

        let ic: f2b::ImageConfig = serde_json::from_str(json_data).unwrap();
        assert_eq!(ic.steer_irq, Some(vec![0, 1]));

        // Also test alias irq_steering
        let json_alias = r#"{
            "vcpus": 2,
            "irq_steering": [2, 3]
        }"#;

        let ic_alias: f2b::ImageConfig = serde_json::from_str(json_alias).unwrap();
        assert_eq!(ic_alias.steer_irq, Some(vec![2, 3]));
    }

    #[test]
    fn test_saved_affinities_roundtrip() {
        let mut affinities: HashMap<u32, String> = HashMap::new();
        affinities.insert(1, "0-3".to_string());
        affinities.insert(12, "0,1".to_string());

        let serialized = serde_json::to_string(&affinities).unwrap();
        let deserialized: HashMap<u32, String> = serde_json::from_str(&serialized).unwrap();

        assert_eq!(affinities, deserialized);
    }
}
