use regex::Regex;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::io;
use std::path::Path;
use std::io::BufRead;

use crate::configGenerator;
use f2b;

const WORKDIR: &str = "/usr/share/runPHI/";
const FPGA_FILE: &str = "fpga_info.txt";

pub fn regions_available(free: &Vec<u32>, requested: &str, assigned: &mut Vec<u32>) -> bool {
    // takes 'list' of regions in form x1-x2-x3 and list of free regions
    // gives back true if are available and modifies 'assigned'
    let regions: Vec<u32> = requested.split('-').map(|s| s.parse::<u32>().unwrap()).collect();   
    for r in &regions {
        //check if free contains region
        if !free.contains(&r){
            return false;
        }
    } 
    for r in regions {
        assigned.push(r);
    }
    return true;
}

pub fn fpgaconf(
    _fc: &f2b::FrontendConfig,
    c: &mut configGenerator::Backendconfig,
    accelerator: &mut f2b::Accelerator
) -> Result<(), Box<dyn Error>> {
    
    let mut file = File::open("/sys/devices/jailhouse/cells/0/fpga_regions_assigned_list")?;
    let mut output_str = String::new();
    file.read_to_string(&mut output_str)?;

    let output_str = output_str.trim();
    
    let mut free_fpga_regions: Vec<u32> = Vec::new();

    // regions might not be consecutive
    for part in output_str.split(',').map(str::trim) {
        if part.contains('-') {
            let mut range_parts = part.split('-');
            let start: u32 = range_parts
                .next()
                .ok_or("Failed to parse start of CPU range")?
                .parse()?;
            let end: u32 = range_parts
                .next()
                .ok_or("Failed to parse end of CPU range")?
                .parse()?;
            free_fpga_regions.extend(start..end);
        } else {
            let single_value: u32 = part.parse()?;
            free_fpga_regions.push(single_value);
        }
    }

    let mut regionsassigned: Vec<u32> = Vec::new(); 
    let mut fpga_bitmask: u32 = 0;
    let fpga_regions_file_path = Path::new(WORKDIR).join(FPGA_FILE);
    let file = File::open(&fpga_regions_file_path)?;
    let reader = io::BufReader::new(file);
    let mut lines = reader.lines();
    lines.next(); //skip header line

    for line in lines {
        if let Ok(record_line) = line {
            let values: Vec<&str> = record_line.split(',').collect();
            if values[0] == accelerator.core {
                let nregions : usize = values[1].parse().expect("Failed to parse number"); 
                //this bitstream requires nregions. Check if specified regions are available
                if regions_available(&free_fpga_regions, values[2], &mut regionsassigned) { 
                   //if they are, assign them to this new cell
                    for n in 0..nregions {
                        println!("values[{}]={}",2+n,values[3+n].to_string());
                        accelerator.bitstream.push(values[3+n].to_string());
                        accelerator.region.push(regionsassigned[n] as i64);
                    }
                    c.fpga_regions = nregions; //for the backend
                    break;
                }
            }
        }
    }
    if  c.fpga_regions == 0 {
        return Err("No regions to program for this bitstream".to_owned().into());
    }    

    // Insert line into the config file
    //This is only the size of the .cpus field so always 1
    let pattern1 = r"__u64 cpus\[1\];";
    let pattern2 = r"__u64 rcpus\[1\];"; // ne ho bisogno???
    let linetoinsert = "\t__u64 fpga_regions[1];";

    // Compile a regular expression to match the pattern and insert the cpus
    let re1 = Regex::new(&pattern1)?;
    let re2 = Regex::new(&pattern2)?;
    if let Some(pos) = re1.find(&c.conf) {
        c.conf
            .insert_str(pos.end(), &format!("\n{}\n", linetoinsert));
    } else if let Some(pos) = re2.find(&c.conf) {
        c.conf
        .insert_str(pos.end(), &format!("\n{}\n", linetoinsert));
    }else{
        return Err("\"__u64 cpus[1] or __u64 rcpus[1]\" not found".into());
        //        return Err("\"__u64 cpus[1]\" not found".into());
    }

     for &region in &regionsassigned {
        fpga_bitmask |= 1 << region;
    } 
    
    let hex_str = format!("{:x}", fpga_bitmask);

    c.conf
    .push_str(&format!("\n\t.fpga_regions = {{\n\t\t0x{},\n\t}},\n", hex_str)); //for cpus in 0x

    return Ok(());

}