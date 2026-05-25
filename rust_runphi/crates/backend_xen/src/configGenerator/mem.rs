

use regex::Regex;
use std::error::Error;
use std::io::{self, BufRead};
use std::process::Command;
use std::str;

use crate::configGenerator;

pub fn memconf(
    c: &mut configGenerator::Backendconfig,
    storage_request: &u64,
    ram_request: &u64,
    group_name : &str
) -> Result<(), Box<dyn Error>> {

    let output = Command::new("xl")
    .arg("info")
    .output()
    .expect("Failed to execute xl info");

    let stdout = str::from_utf8(&output.stdout).expect("Invalid UTF-8 in xl info output");

    let mut vfree_mb: Option<u64> = None;

    for line in stdout.lines() {
        if line.trim_start().starts_with("free_memory") {
            if let Some(value_str) = line.split(':').nth(1) {
                vfree_mb = value_str.trim().parse::<u64>().ok();
            }
        }
    }

    match vfree_mb {
        Some(value) => logging::log_message(logging::Level::Info, &format!("Free memory: {} MB", value)),
        None => eprintln!("Could not find 'free_memory' in xl info output"),
    }
    
    if vfree_mb == Some(0) {
        logging::log_message(logging::Level::Error, "No free memory available in the specified volume group.");
        return Err("Error no mem free".to_owned().into());
    }

    //Ceck that there is enaugh mem
    if let Some(free_mb) = vfree_mb {
        if *storage_request > free_mb {   
            logging::log_message(logging::Level::Error, "Not enough free memory in the specified volume group.");
            return Err("Error not enough mem free".to_owned().into());        
        }
    } else {
        logging::log_message(logging::Level::Error, "Could not determine free memory.");
        return Err("Error could not determine free memory".to_owned().into());
    }


    //Build the name of lvm like "lv_containerID"
    let mut lvm_name: String = String ::from("lv_"); 

    let pattern = r#"name\s*=\s*"([^"]+)""#;
    let re = Regex::new(&pattern).unwrap();
    let reader =io::Cursor::new(c.conf.clone());

    for line in reader.lines() {
        let line = line?; 

        if let Some(captures) = re.captures(&line) {
            let name = captures.get(1).unwrap().as_str();
            lvm_name.push_str(name);
            break;
        }
    }

    //build the path of vgs to put in config file
    let mut vgs = String::from("/dev/");
    vgs.push_str(group_name);

    if *storage_request == 0{
        c.conf
            .push_str(&format!("\n#storage_request = {}\ndisk = ['{}/{},raw,xvda,rw']\n","1024M",
                                       vgs,&lvm_name));
    }else{
        let volume_size_str = format!("{}M", storage_request);
        //c.conf.push_str(&format!("\n#storage_request = {} \ndisk = ['{}/{},raw,xvda,rw']\n",volume_size_str,vgs,&lvm_name));
        //Remove disk line from config file
        c.conf.push_str(&format!("#storage_request = {}",volume_size_str));
    }



    //Put the memory request in the config file
    if *ram_request == 0{

        c.conf
            .push_str(&format!("\n#Initial memory and max memory\nmemory =  1024 \nmaxmem = 1024\n"));

    }else {

        c.conf
            .push_str(&format!("\n#Initial memory and max memory\nmemory = {} \nmaxmem = {}\n",
                      ram_request,ram_request));

    }

    return Ok(());
}
