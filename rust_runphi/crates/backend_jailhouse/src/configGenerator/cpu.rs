//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (f.boccola@studenti.unina.it)
//*********************************************

use regex::Regex;
use std::error::Error;
//use std::fs::{self, File, OpenOptions};
use std::fs::File;
//use std::io::{Read, Seek, SeekFrom, Write};
use std::io::Read;
//use std::process::{Command, Stdio};

use crate::configGenerator;
use f2b;

//const WORKPATH: &str = "/usr/share/runPHI";
//const RUNDIR: &str = "/run/runPHI";

pub fn cpuconf(
    _fc: &f2b::FrontendConfig,
    c: &mut configGenerator::Backendconfig,
    _quota: &f64,
    _period: &f64,
    cpusf64: &f64,
) -> Result<(), Box<dyn Error>> {
    /* let mut logfile = fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open("/usr/share/runPHI/log_CPU.txt")?; */

    let cpus: u8 = cpusf64.ceil() as u8; // Casting with ceil due to jh not supporting fraction of cpu allocation
    c.cpus = cpus;

    let mut file = File::open("/sys/devices/jailhouse/cells/0/cpus_assigned_list")?;
    let mut output_str = String::new();
    file.read_to_string(&mut output_str)?;

    // Trim any trailing whitespace/newlines
    let output_str = output_str.trim();

    // Split the range by '-' and parse the numbers
    let mut parts = output_str.split('-');
    let start: usize = parts
        .next()
        .ok_or("Failed to parse start of CPU range")?
        .parse()?;
    let end: usize = parts
        .next()
        .ok_or("Failed to parse end of CPU range: just the root cell CPU left")?
        .parse()?;

    // Create the free_cpus vector excluding the last CPU in the range
    let free_cpus: Vec<usize> = (start..end).collect();

    // Ensure there are enough available CPUs
    if free_cpus.len() < cpus as usize {
        return Err("Not enough free CPU left".to_owned().into());
    }

    //writeln!(logfile, "free_cpus: {:?}", free_cpus)?; //DEBUG

    // Assign the required number of CPUs
    let cpusassigned: Vec<usize> = free_cpus[..cpus as usize].to_vec();
    //writeln!(logfile, "cpuassigned: {:?}", cpusassigned)?; //DEBUG

    // Insert line into the config file
    //This is only the size of the .cpus field so always 1
    let pattern = "struct jailhouse_cell_desc cell;";
    let linetoinsert = "\t__u64 cpus[1];";

    // Compile a regular expression to match the pattern and insert the cpus
    let re = Regex::new(&pattern)?;
    if let Some(pos) = re.find(&c.conf) {
        c.conf
            .insert_str(pos.end(), &format!("\n{}\n", linetoinsert));
    } else {
        return Err("\"struct jailhouse_cell_desc cell\" not found".into());
    }

    // Append Cpuarray
    // A binary string representation of the assigned CPUs is created.
    //The binary string is formatted and appended to the configuration string (c.conf)
    let mut cpus_bitmask: u64 = 0;
    for &cpu in &cpusassigned {
        cpus_bitmask |= 1 << cpu;
    }

    let hex_str = format!("{:x}", cpus_bitmask); //for cpus in 0x

    //let binary_str = format!("{:b}", cpus_bitmask); //for cpus in 0b

    //writeln!(logfile, "binary string: {}", binary_str)?; //DEBUG

    //c.conf.push_str(&format!("\n\t.cpus = {{\n\t\t0b{},\n\t}},\n", binary_str)); //for cpus in 0b
    c.conf
        .push_str(&format!("\n\t.cpus = {{\n\t\t0x{},\n\t}},\n", hex_str)); //for cpus in 0x

    //writeln!(logfile, "last line of cpuconf")?; //DEBUG

    return Ok(());
}
