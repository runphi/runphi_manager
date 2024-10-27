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


/**
if the requested regions are available, remove them from free and give them to assigned
return number of regions assigned

if they are not available, do not modify free or assigned and return 0
*/
pub fn regions_available(free: &mut Vec<u32>, requested: &str, assigned: &mut Vec<u32>) -> usize {
    let regions: Vec<u32> = requested.split('-').map(|s| s.parse::<u32>().unwrap()).collect();   
    for r in &regions {
        //check if free contains region
        if !free.contains(&r){
            return 0;
        }
    } 
    for r in &regions {
        assigned.push(*r);
    }
    //remove from free
    free.retain(|x| !assigned.contains(x));
    return regions.len();
}

pub fn fpgaconf(
    _fc: &f2b::FrontendConfig,
    c: &mut configGenerator::Backendconfig,
    config: &mut f2b::ImageConfig
) -> Result<(), Box<dyn Error>> {

    // Check which regions are available
    let mut file = File::open("/sys/devices/jailhouse/cells/0/fpga_regions_assigned_list")?;
    let mut output_str = String::new();
    file.read_to_string(&mut output_str)?;

    let output_str = output_str.trim();
    
    let mut free_fpga_regions: Vec<u32> = Vec::new();

    // regions might not be consecutive!
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
            free_fpga_regions.extend(start..=end);
        } else {
            let single_value: u32 = part.parse()?;
            free_fpga_regions.push(single_value);
        }
    }

    let mut regionsassigned: Vec<u32> = Vec::new(); 
    let mut fpga_bitmask: u32 = 0;
    let mut nregions = 0;

    // read fpga_info.txt file, which contains info about locally available bitstreams
    let fpga_regions_file_path = Path::new(WORKDIR).join(FPGA_FILE);
    let file = File::open(&fpga_regions_file_path)?;
    let mut lines : Vec<String> =  io::BufReader::new(&file).lines()
                    .collect::<Result<_, _>>()?;
    lines.remove(0);//skip header
    
    for accelerator in &config.accelerators {
        // for each accelerator
        if accelerator.bitstream.is_empty(){  // if we were given core name, and not bstream directly
            for line in &lines{
                let values: Vec<&str> = line.split(',').collect();                
                if values[0] == accelerator.core {
                    nregions = regions_available(&mut free_fpga_regions, values[1], &mut regionsassigned);
                    if nregions != 0 {
                        for n in 0..nregions {
                            config.bitstreams.push(values[2+n].to_string());
                            config.regions.push(regionsassigned[regionsassigned.len()-nregions + n] as i64); //last n assigned
                        }
                        c.fpga_regions+=nregions;
                        break;
                    }
                }
            }
        } else{ // we were given bitstream inside the path
            let file_name = Path::new(&accelerator.bitstream) //we were given bitstream path!
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap();

            nregions = regions_available(&mut free_fpga_regions, &accelerator.region, &mut regionsassigned);
            // it's either 1 or 0
            if nregions != 0 {
                config.bitstreams.push(file_name.to_string());
                config.regions.push(regionsassigned[regionsassigned.len()-1] as i64); //last one
                c.fpga_regions+=1;
            }
        }    
         if nregions == 0{ 
            // this accelerator can't be loaded
            return Err("Unable to program FPGA: not enough space available or bitstream not found".to_owned().into());
        }    
    }
 

    // Insert values into config! 

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
