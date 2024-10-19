//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (f.boccola@studenti.unina.it)
//*********************************************

use serde::Deserialize;
use serde_json;
use std::fs;

// This structure holds all the information mapped from the cli
// That basically means that are the flags from the OCI spec. We could pass directly
// the OCI structures, however a buffer structure like this allows for data modification
// before passing on to the backend (like cutting the ID down to 24 chars), and the
// backend only depends on this structure, that is much easier to control than the entire
// OCI structures (multiple variables do not actually make a lot of sense to consider
// in a partitioned container)
pub struct FrontendConfig {
    // Jsonconfig is the json parsed coming from the upper layer
    pub jsonconfig: serde_json::Value,
    pub crundir: String,
    pub containerid: String,
    pub bundle: String,
    pub mountpoint: String,
    pub guestconsole: String,
    pub pidfile: String,
}
impl FrontendConfig {
    pub fn new() -> Self {
        Self {
            jsonconfig: serde_json::Value::Null,
            crundir: String::new(),
            containerid: String::new(),
            bundle: String::new(),
            mountpoint: String::new(),
            guestconsole: String::new(),
            pidfile: String::new(),
        }
    }
}


//Structure to hold information about an hardware accelerator
#[derive(Debug, Deserialize)]
pub struct Accelerator {
    #[serde(default)]
    pub core: String,

    #[serde(rename = "starting_vaddress",default)]
    pub acc_starting_vaddress: String, //for this accelerator, if necessary

    #[serde(rename = "inmate",default)]
    pub acc_inmate: String,

    //If the image contains its own bitstream:
    #[serde[default]] 
    pub bitstream: String,
    
    #[serde[default]] 
    pub region: String,
    
}

impl Default for Accelerator {
    fn default() -> Self {
        Accelerator {
            core: String::new(),
            acc_starting_vaddress: String::new(),
            acc_inmate: String::new(),
            bitstream: String::new(),
            region :String::new(),
        }
    }
}
// This structure holds the information that describe the image to be started as partitioned cell
// These are additional to standard information required by containers. For example, if dealing with a
// binary, the starting virtual address is required to perform a mapping, or the devices used or the
// binary to load in the cell
#[derive(Debug, Deserialize)]
pub struct ImageConfig {
    #[serde(default)]
    // Check if the container comes with its own Kernel and/or Ramdisk
    // runPHI requires that both the kernel and initrd are exposed by client through container's env variables KERNEL=/path/to/kernel_image, RAMDISK/path/to/initrd
    // moreover, in case of Jailhouse, the user should provide the path, in the container fs, of the inmate to run
    pub cpio: String,
    #[serde(default)]
    // OSvar stores information in a file /OS to indentify the OS to load
    // if OSvar is "linux", the OS file contains the image of the linux kernel
    // if the OSvar contains "zephyr", zephyr is loaded
    // NOTE: ----------------------------------------
    // TODO: The OS var for zephyr is not actually needed, since zephyr is a de-facto
    // bare metal inmate. At the moment, however, is needed to mockup the .cell file loaded
    // In any case it is good to keep the OS information somewhere for future stuff.
    pub os_var: String,
    #[serde(default)]
    pub kernel: String,
    #[serde(default)]
    pub ramdisk: String,
    #[serde(default)]
    pub inmate: String,
    #[serde(default)]
    pub dtb: String,
    #[serde(default)]
    pub initrd: String,
    #[serde(default)]
    pub netconf: String,
    #[serde(default)]
    pub starting_vaddress: String,
    #[serde(default)]
    //TODO: handle default or missing values in a decent way
    // This line is needed to include the "net" field
    pub net: String,
    //if the image contains the description of an accelerator
    #[serde[default]] 
    //pub accelerator: Accelerator
    pub accelerators: Vec<Accelerator>,

    #[serde(skip)]
    pub bitstreams: Vec<String>,

    #[serde(skip)]
    pub regions : Vec<i64>

} 
impl ImageConfig {
    pub fn get_from_file(mountpoint: &str) -> Self {

        // parsing configuration variables from the file
        //TODO: here is the case to parse also a node default used in the case the container does not specify this
        //TODO: parametrize boot boot.bin and config.json
        let json_str = match fs::read_to_string(format!("{}/boot/config.json", mountpoint)) {
            Ok(content) => content,
            Err(_) => String::new(),
        };
        let mut config: ImageConfig = serde_json::from_str(&json_str).unwrap();
/* 
        // FOR NOW
         //config.accelerator.core = format!("simple");
         let mut a : Accelerator = Accelerator::default();
         a.core = "simple".to_string();
         a.acc_inmate = "/boot/hi.bin".to_string();
         a.acc_starting_vaddress = "0x2".to_string();
         let mut a1 : Accelerator = Accelerator::default();
         a1.bitstream = "/boot/bitstream/anna.bit".to_string();
         a1.region = "0".to_string(); 
         let mut a2 : Accelerator = Accelerator::default();
         a2.core="big-soft-core".to_string();
         a2.acc_inmate = "/boot/hibig.bin".to_string();
         a2.acc_starting_vaddress = "0x4".to_string();
         config.accelerators.push(a1); //first request region 0
         config.accelerators.push(a2);  //then request region 1 and 2
         config.accelerators.push(a); //ther request region 0 or 3 (will take 3)
  */

        if !config.inmate.is_empty() {
            config.inmate = format!("{}{}", mountpoint, config.inmate);
        } else {
            config.inmate = format!("{}/boot/boot.bin", mountpoint);
        }
        for accelerator in &mut config.accelerators {
            if !accelerator.bitstream.is_empty(){
                accelerator.bitstream = format!("{}{}", mountpoint, accelerator.bitstream);
            }
            if !accelerator.acc_inmate.is_empty() {
                accelerator.acc_inmate = format!("{}{}", mountpoint, accelerator.acc_inmate);
            }
        }
       
        return config;
    }
}
