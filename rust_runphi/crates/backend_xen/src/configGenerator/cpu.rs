

use std::error::Error;
use std::process::Command;

use crate::configGenerator;
use f2b;


pub fn cpuconf(
    _fc: &f2b::FrontendConfig,
    c: &mut configGenerator::Backendconfig,
    _quota: &f64,
    _period: &f64,
    cpusf64: &f64,
) -> Result<(), Box<dyn Error>> {


    let cpus: u8 = cpusf64.ceil() as u8; // Casting with ceil due to xen not supporting fraction of cpu allocation
    c.cpus = cpus;
    let mut tot_cpus: Vec<i64> = Vec::new();

    //Take the info about free CPUs

    //Take the info about all the pCPUs
    let lscpu_output = Command::new("lscpu")
    .output()
    .expect("Errore durante l'esecuzione del comando lscpu");
    
    if lscpu_output.status.success() {
        let stdout = String::from_utf8_lossy(&lscpu_output.stdout);

        for line in stdout.lines() {
            if line.starts_with("CPU(s):") {
                let cpus: i64 = line.split_whitespace().nth(1).unwrap().parse().expect("Failed to parse cpu value");
                tot_cpus.extend(0..cpus);
                break;
            }
        }
    } else {
        eprintln!("Errore: {}", String::from_utf8_lossy(&lscpu_output.stderr));
    }


    //take the info about the already pinned
    let output = Command::new("xl")
        .arg("vcpu-list")  
        .output()   
        .expect("Error during xl execution");


    // Name                                ID  VCPU   CPU State   Time(s) Affinity (Hard / Soft)
    // Domain-0                             0     0    4   -b-     207.6  0 / all
    // Domain-0                             0     1    4   -b-     205.5  1 / all
    // Guest1                               1     0    7   r--     150.9  7 / 7
    // Guest2                               2     0    6   r--     150.9  6 / 6

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
            
        let mut assigned_cpus: Vec<i64> = Vec::new();

        for line in stdout.lines() {
            if line.contains("Name"){
                continue;
            }

            let columns: Vec<&str> = line.split_whitespace().collect();


            //take the pCPU pinned by Guest
            if line.contains("Domain-0"){
                continue;
            }else {
                if let Ok(cpu) = columns[3].parse::<i64>() {
                    assigned_cpus.push(cpu);
                }
            }
        }

        //Get the free cpu
        let free_cpus: Vec<i64> = tot_cpus.into_iter()
                                              .filter(|x|  !assigned_cpus.contains(x))
                                              .collect();


        // Ensure there are enough available CPUs
        if free_cpus.len()+1 < cpus as usize {
            return Err("Not enough free CPU left".to_owned().into());
        }
        
        // Assign the required number of CPUs
        let cpus2assign: Vec<i64> = free_cpus[free_cpus.len() - cpus as usize ..].to_vec();

        c.conf
            .push_str(&format!("\nvcpus = {}\n\ncpus = {:?} \n\ncpu_affinity = {:?}\n\n",
                      cpus, cpus2assign, cpus2assign.iter().map(|x| x.to_string()).collect::<Vec<String>>()));

    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Error: {}", stderr);
    }

    return Ok(());
}
