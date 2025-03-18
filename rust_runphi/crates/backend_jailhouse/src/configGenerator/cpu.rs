//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

//use regex::Regex;
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

    // Assign the required number of CPUs
    let cpusassigned: Vec<usize> = free_cpus[..cpus as usize].to_vec();

    // Append Cpuarray
    // A binary string representation of the assigned CPUs is created.
    //The binary string is formatted and appended to the configuration string (c.conf)
    let mut cpus_bitmask: u64 = 0;
    for &cpu in &cpusassigned {
        cpus_bitmask |= 1 << cpu;
    }

    let hex_str = format!("{:x}", cpus_bitmask); //for cpus in 0x

    //let binary_str = format!("{:b}", cpus_bitmask); //for cpus in 0b

    //c.conf.push_str(&format!("\n\t.cpus = {{\n\t\t0b{},\n\t}},\n", binary_str)); //for cpus in 0b
    c.conf
        .push_str(&format!("\n\t.cpus = {{\n\t\t0x{},\n\t}},\n", hex_str)); //for cpus in 0x

    return Ok(());
}
