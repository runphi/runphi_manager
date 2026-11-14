//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

use std::error::Error;
use std::fs;

use crate::configGenerator;
use f2b;

const ROOT_CELL_CPUS: &str = "/sys/devices/jailhouse/cells/0/cpus_assigned_list";

// Parse a CPU list in the kernel's format ("0-3", "1,3", "0,2-3").
fn parse_cpu_list(list: &str) -> Result<Vec<usize>, Box<dyn Error>> {
    let mut cpus = Vec::new();
    for part in list.trim().split(',').filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((start, end)) => {
                let (start, end): (usize, usize) = (start.trim().parse()?, end.trim().parse()?);
                cpus.extend(start..=end);
            }
            None => cpus.push(part.trim().parse()?),
        }
    }
    cpus.sort_unstable();
    cpus.dedup();
    Ok(cpus)
}

pub fn cpuconf(
    _fc: &f2b::FrontendConfig,
    c: &mut configGenerator::BackendConfig,
    _quota: &f64,
    _period: &f64,
    cpusf64: &f64,
) -> Result<(), Box<dyn Error>> {

    let cpus: u8 = cpusf64.ceil() as u8; // Casting with ceil due to jh not supporting fraction of cpu allocation
    c.cpus = cpus;

    let mut cpus_bitmask: u64 = 0;
    if cpus > 0 {
        // The CPUs of the root cell, but its last one, which the root cell keeps
        let root_cpus = parse_cpu_list(&fs::read_to_string(ROOT_CELL_CPUS)?)?;
        let free_cpus = &root_cpus[..root_cpus.len().saturating_sub(1)];

        if free_cpus.len() < cpus as usize {
            return Err(format!(
                "not enough free CPUs: need {}, the root cell can give {} ({:?})",
                cpus,
                free_cpus.len(),
                free_cpus
            )
            .into());
        }

        for &cpu in &free_cpus[..cpus as usize] {
            cpus_bitmask |= 1 << cpu;
        }
    }

    // Append Cpuarray (an R5 cell has no A53 CPU: an empty set)
    c.conf
        .push_str(&format!("\n\t.cpus = {{\n\t\t0x{:x},\n\t}},\n", cpus_bitmask));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_cpu_list;

    #[test]
    fn cpu_lists() {
        assert_eq!(parse_cpu_list("0-3\n").unwrap(), vec![0, 1, 2, 3]);
        assert_eq!(parse_cpu_list("1-3").unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_cpu_list("0,2-3").unwrap(), vec![0, 2, 3]);
        assert_eq!(parse_cpu_list("3").unwrap(), vec![3]);
        assert!(parse_cpu_list("x").is_err());
    }
}
