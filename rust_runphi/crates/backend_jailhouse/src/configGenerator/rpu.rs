//*********************************************
// Authors: Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

//use regex::Regex;
use std::error::Error;
//use std::fs::{self, File, OpenOptions};
//use std::fs;
//use std::io::{Read, Seek, SeekFrom, Write};
//use std::io::Read;
//use std::process::{Command, Stdio};
//use std::path::Path;

use crate::configGenerator;
//use f2b;

//const WORKPATH: &str = "/usr/share/runPHI";
//const RUNDIR: &str = "/run/runPHI";

pub fn rpuconf(
    c: &mut configGenerator::Backendconfig,
    rpusf64: &f64,
) -> Result<(), Box<dyn Error>> {
    // cpus will be the effective number of RPUs requested
    let cpus: u8 = rpusf64.ceil() as u8; // Casting with ceil since fractional CPU allocation isn't supported

    // Construct the full path to the free_rpus.txt file
    let mut free_rpus = c.rcpus.clone();

    // Check if there are enough free RPUs to fulfill the request
    if free_rpus.len() < cpus as usize {
        return Err("Not enough free RPUs available".into());
    }

    // Allocate the requested number of RPUs
    let allocated_rpus: Vec<i8> = free_rpus.drain(0..cpus as usize).collect();

    c.used_rcpus = allocated_rpus.clone();

    // Update `c.rcpus` with the remaining RPUs
    c.rcpus = free_rpus;

    // Create the rpus assignment with allocated RPUs in the required format
    let rpus_hex: Vec<String> = allocated_rpus
        .iter()
        .map(|rpu| format!("0x{:x}", rpu + 1)) // Add 1 to each RPU ID for the correct hex representation
        .collect();
    let rpus_assignment = format!("\n\t.rcpus = {{\n\t\t{},\n\t}},\n", rpus_hex.join(", "));

    // Append the RPU configuration to the config
    c.conf.push_str(&rpus_assignment);

    Ok(())
}