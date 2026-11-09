

use std::error::Error;
use std::process::Command;

use crate::configGenerator;
use f2b;


// Physical CPUs of the host, as Xen counts them: "nr_cpus" of `xl info`.
// Dom0 sees only its own vCPUs (dom0_max_vcpus), so lscpu cannot be used.
fn parse_nr_cpus(xl_info: &str) -> Option<i64> {
    xl_info
        .lines()
        .find(|line| line.split(':').next().map(str::trim) == Some("nr_cpus"))
        .and_then(|line| line.split(':').nth(1))
        .and_then(|value| value.trim().parse().ok())
}

// A cpulist ("3", "2-3", "0,2-3"), as in xl's affinity column.
fn parse_cpulist(list: &str) -> Vec<i64> {
    let mut cpus = Vec::new();
    for part in list.split(',') {
        match part.split_once('-') {
            Some((first, last)) => {
                if let (Ok(first), Ok(last)) = (first.parse::<i64>(), last.parse::<i64>()) {
                    cpus.extend(first..=last);
                }
            }
            None => {
                if let Ok(cpu) = part.parse::<i64>() {
                    cpus.push(cpu);
                }
            }
        }
    }
    cpus
}

// Physical CPUs that are not free for a new domain, from `xl vcpu-list`:
// the hard affinity of every vCPU of the other domains, which is known as
// soon as a domain exists, also while it is paused (xl create -p), unlike
// the CPU a vCPU last ran on. Dom0 keeps CPUs for its own vCPUs: the ones
// they are pinned to, or, if they run anywhere ("all"), the lowest ones, one
// per vCPU. A guest on Dom0's CPU would starve Dom0: with vwfi=native a vCPU
// waiting for interrupts does not give its CPU back to Xen.
//
// Name                                ID  VCPU   CPU State   Time(s) Affinity (Hard / Soft)
// Domain-0                             0     0    0   r--      42.1  all / all
// 5d6fd3165ce9a8a4f7670a51             2     0    -   --p       0.0  3 / all
fn parse_reserved_cpus(xl_vcpu_list: &str) -> Vec<i64> {
    let mut reserved = Vec::new();
    let mut dom0_unpinned = 0;
    for line in xl_vcpu_list.lines() {
        let columns: Vec<&str> = line.split_whitespace().collect();
        if columns.len() < 7 || columns[0] == "Name" {
            continue;
        }
        if columns[0] == "Domain-0" && columns[6] == "all" {
            dom0_unpinned += 1;
        } else {
            reserved.extend(parse_cpulist(columns[6]));
        }
    }
    let mut cpu = 0;
    while dom0_unpinned > 0 {
        if !reserved.contains(&cpu) {
            reserved.push(cpu);
            dom0_unpinned -= 1;
        }
        cpu += 1;
    }
    reserved
}

// The `cpus` highest physical CPUs that are not reserved.
fn choose_cpus(nr_cpus: i64, reserved: &[i64], cpus: usize) -> Result<Vec<i64>, String> {
    let free: Vec<i64> = (0..nr_cpus).filter(|cpu| !reserved.contains(cpu)).collect();
    if free.len() < cpus {
        return Err(format!(
            "not enough free CPUs: need {}, have {} of {} ({:?} for Dom0 and the other domains)",
            cpus,
            free.len(),
            nr_cpus,
            reserved
        ));
    }
    Ok(free[free.len() - cpus..].to_vec())
}

fn xl(args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = Command::new("xl").args(args).output()?;
    if !output.status.success() {
        return Err(format!(
            "xl {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn cpuconf(
    _fc: &f2b::FrontendConfig,
    c: &mut configGenerator::BackendConfig,
    _quota: &f64,
    _period: &f64,
    cpusf64: &f64,
) -> Result<(), Box<dyn Error>> {


    let cpus: u8 = cpusf64.ceil() as u8; // Casting with ceil due to xen not supporting fraction of cpu allocation
    c.cpus = cpus;

    let nr_cpus = parse_nr_cpus(&xl(&["info"])?).ok_or("xl info: no nr_cpus")?;
    let reserved = parse_reserved_cpus(&xl(&["vcpu-list"])?);
    let cpus2assign = choose_cpus(nr_cpus, &reserved, cpus as usize)?;

    c.conf
        .push_str(&format!("vcpus = {}\ncpus = {:?} \ncpu_affinity = {:?}\n",
                  cpus, cpus2assign, cpus2assign.iter().map(|x| x.to_string()).collect::<Vec<String>>()));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nr_cpus_from_xl_info() {
        let info = "host                   : qemu-xen\nrelease                : 6.1.70\nnr_cpus                : 4\nmax_cpu_id             : 3\n";
        assert_eq!(parse_nr_cpus(info), Some(4));
        assert_eq!(parse_nr_cpus("host : x\n"), None);
    }

    #[test]
    fn cpulists() {
        assert_eq!(parse_cpulist("3"), vec![3]);
        assert_eq!(parse_cpulist("2-3"), vec![2, 3]);
        assert_eq!(parse_cpulist("0,2-3"), vec![0, 2, 3]);
        assert_eq!(parse_cpulist("all"), Vec::<i64>::new());
    }

    // An unpinned Dom0 vCPU (affinity all) reserves the lowest free CPU; a
    // paused guest ("-" as CPU) still counts through its affinity.
    #[test]
    fn reserved_cpus_from_vcpu_list() {
        let list = "\
Name                                ID  VCPU   CPU State   Time(s) Affinity (Hard / Soft)
Domain-0                             0     0    0   r--      42.1  all / all
5d6fd3165ce9a8a4f7670a51             2     0    3   -b-       5.0  3 / all
e9315f7578449fb66686aa07             3     0    -   --p       0.0  1-2 / all
";
        assert_eq!(parse_reserved_cpus(list), vec![3, 1, 2, 0]);
        let header = "Name                                ID  VCPU   CPU State   Time(s) Affinity (Hard / Soft)\n";
        // two unpinned Dom0 vCPUs: CPUs 0 and 1
        let dom0 = "Domain-0 0 0 0 r-- 1.0 all / all\nDomain-0 0 1 1 -b- 1.0 all / all\n";
        assert_eq!(parse_reserved_cpus(&format!("{}{}", header, dom0)), vec![0, 1]);
        // a pinned Dom0 vCPU reserves its own CPU
        let pinned = "Domain-0 0 0 2 r-- 1.0 2 / all\n";
        assert_eq!(parse_reserved_cpus(&format!("{}{}", header, pinned)), vec![2]);
    }

    // The highest free CPUs first: Dom0 keeps the lowest.
    #[test]
    fn choose_highest_free_cpus() {
        assert_eq!(choose_cpus(4, &[0], 1).unwrap(), vec![3]);
        assert_eq!(choose_cpus(4, &[0, 3], 2).unwrap(), vec![1, 2]);
        assert_eq!(choose_cpus(4, &[0, 2, 3], 1).unwrap(), vec![1]);
    }

    // Asking for more CPUs than are free is an error, not a panic (the old
    // check let one CPU too many through, and the slice underflowed).
    #[test]
    fn not_enough_free_cpus() {
        let err = choose_cpus(4, &[0, 1, 2, 3], 1).unwrap_err();
        assert!(err.contains("not enough free CPUs: need 1, have 0 of 4"), "{}", err);
        assert!(choose_cpus(4, &[0, 3], 3).is_err());
    }
}
