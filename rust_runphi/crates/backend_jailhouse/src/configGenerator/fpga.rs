use regex::Regex;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::io;
use std::path::Path;
use std::io::BufRead;


use crate::configGenerator;
use f2b;

const FPGADIR: &str = "/usr/share/runPHI/fpga_dir/";
const FPGA_FILE: &str = "fpga_info.txt";

/*
pub fn qualcos(){
    //apri quel file
    //per ogni linea in quel file
    //se il nome (il primo elemento) è uguale al nome del bitstream che ti serve
    //allora controlla se le regioni in questione sono libere
    //se si, metti c.fpga_regions = num, modifica regions_assigned
    //metti da qualche parte che il nome del bstream è quello (magari sostituisci il nome preciso a core nell'image config)
    }

*/

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

    //chiama qui la funzione, passa free_..., cambia regions_assigned (forse ritorna? forse direttamente come bitmask)
    // Ensure there are enough available regions
   /*  if free_fpga_regions.len() < fpga_regions as usize {
        return Err("Not enough free regions left".to_owned().into());
    }    
 */
    //let regionsassigned: Vec<usize> = free_fpga_regions[..fpga_regions as usize].to_vec();   
    let mut fpga_bitmask: u32 = 0;
    let fpga_regions_file_path = Path::new(FPGADIR).join(FPGA_FILE);
    let mut file = File::open(&fpga_regions_file_path)?;
    let reader = io::BufReader::new(file);
    let mut lines = reader.lines();
    lines.next(); //skip header line

    for line in lines {
        if let Ok(record_line) = line {
            let values: Vec<&str> = record_line.split(',').collect();
            //println!("{}=={}?",values[0],core);
            if values[0] == accelerator.core {
                let region : u32 = values[1].parse().expect("Failed to parse number");
                if free_fpga_regions.contains(&region){
                    accelerator.bitstream =  values[2].to_string(); 
                    accelerator.region = region as i64;
                    //aggiungi anche a regionsassigned in futuro
                    c.fpga_regions = 1;
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

    let mut fpga_bitmask: u64 = 0;
    //for now, just one region. then, vector regionsassigned
    fpga_bitmask |= 1 << accelerator.region;
   /*  for &region in &regionsassigned {
        fpga_bitmask |= 1 << region;
    } */
    
    let hex_str = format!("{:x}", fpga_bitmask);

    c.conf
    .push_str(&format!("\n\t.fpga_regions = {{\n\t\t0x{},\n\t}},\n", hex_str)); //for cpus in 0x

    return Ok(());

}