

use regex::Regex;
use std::error::Error;
use std::io::{self, BufRead};
use std::process::Command;

use crate::configGenerator;

pub fn memconf(
    c: &mut configGenerator::Backendconfig,
    storage_request: &u64,
    ram_request: &u64,
    group_name : &str
) -> Result<(), Box<dyn Error>> {

    let mut vfree_mb: u64 = 0;

    //Get the free space from the volume group name in input 
    let output = Command::new("vgs") 
        .output()   
        .expect("Error during vgs execution");

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        if line.contains("VG"){
            continue;
        }

        let columns: Vec<&str> = line.split_whitespace().collect();

        if line.contains(group_name){
            if let Some(VFree) = columns.get(6){
                let vfree_gb = VFree.trim_end_matches('g').replace(",", ".");
                if let Ok(vfree_gb_value) = vfree_gb.parse::<f64>() {
                    vfree_mb = (vfree_gb_value * 1000.0).round() as u64; // GB to MB
                } else {
                    println!("Error parsing value of VFree.");
                }
            }
        }
    }

    if vfree_mb == 0 {
        return Err("Error no mem free".to_owned().into());
    }

    //Ceck that there is enaugh mem
    if *storage_request > vfree_mb {   
        return Err("Error not enough mem free".to_owned().into());        
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
        c.conf
            .push_str(&format!("\n#storage_request = {} \ndisk = ['{}/{},raw,xvda,rw']\n",
                                       volume_size_str,vgs,&lvm_name));
    }



    //Put the memory rewuest in the config file
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
