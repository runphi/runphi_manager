
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::process::Command;
use std::time::{SystemTime, Duration};

use f2b;
pub mod boot;
pub mod communication;
pub mod cpu;
pub mod device;
pub mod mem;
pub mod network;

//const WORKPATH: &str = "/usr/share/runPHI";
//const RUNDIR: &str = "/run/runPHI";
//(fc.crundir = run/runPHI/containerid)
//in questo caso stiamo utilizzando LVM per gestire i dischi guest
const LVM_GROUP_NAME: &str = "test-vg";

// This structure holds all the information related to the configuration of the partitioned container
// There is the configuration file, the configuration string, and needed variables for resources,
// like cpus, memory addresses, devices, and in general all the output of the configGeneration phase
#[derive(Debug)]
pub struct Backendconfig {
    pub conf: String,
    pub cpus: u8,
    pub conffile: String,
    pub net: String,
}

impl Backendconfig {
    // Constructor function
    pub fn new() -> Self {
        Self {
            conf: String::new(),
            cpus: 0,
            conffile: String::new(),
            net: String::new(),
        }
    }
}

//TODO: error handling across this function is a box of shit, handle it
pub fn config_generate(fc: &f2b::FrontendConfig) -> Result<Box<f2b::ImageConfig>, Box<dyn Error>> {

    let start_time = SystemTime::now();

    let _ = append_message_with_time(&format!("starting config generator")); //TIME
    let mut c = Backendconfig::new();
    c.conffile = format!("{}/config.cfg", fc.crundir);
    let _ = append_message_with_time(&format!("Target file path : {}",c.conffile)); //TIME

    // parsing configuration variables from the file
    //THIS IS THE ACCESS TO JSON.CONFIG FROM DOCKER
    //writeln!(logfile, "Parsing config.json")?;                        //DEBUG
    let mut config = Box::new(f2b::ImageConfig::get_from_file(&fc.mountpoint));

    

    //let start_time = Instant::now();                                                    //TIME
    let _ = append_message_with_time(&format!("helper start")); //TIME
    let _ = confighelperstart(fc, &mut c, &config);
    let _ = append_message_with_time(&format!("helper start end")); //TIME
    //let _ = append_message_with_time(&format!("Time elapsed in helper start is: {:?}", start_time.elapsed())); //TIME

    let _ = append_message_with_time(&format!("boot config start")); //TIME
    let _ = boot::bootconf(fc, &mut c, &mut config);
    let _ = append_message_with_time(&format!("boot config end")); //TIME


    // This region of code could be extended with code to retrieve other specific Docker's flags which set CPU limitations
    // cpus where allow guest execution set by Docker's flag 'cpuset-cpus'
    // If flag is not set, let's go for 1. It will be overwritten by quota and period if they are defined
    let _cpu_set = fc.jsonconfig["linux"]["resources"]["cpu"]["cpus"]
        .as_f64()
        .unwrap_or(1.0);

    //writeln!(logfile, "Got cpu_set")?;                    //DEBUG

    //Through Docker's flag "cpus=0.0000" user requires an amount cpus usage as percentage
    //That percentage will be expressed in form of quota-period ratio (EG: cpus=2.00 means values:
    // quota=20000 and period=10000 --> cpus=2)
    // Extract period and quota as floats
    // Set default value to 1.0 if not present or not a float
    let period = fc.jsonconfig["linux"]["resources"]["cpu"]["period"]
        .as_f64()
        .unwrap_or(10000.0);

    // Set default value to 1.0 if not present or not a float
    let quota = fc.jsonconfig["linux"]["resources"]["cpu"]["quota"]
        .as_f64()
        .unwrap_or(10000.0);

    //writeln!(logfile, "Got period quota {} {}", period, quota)?;       //DEBUG

    // cpus is a floating point number
    // If the backend does not support fractional allots, that's a backend matter
    let cpus: f64 = quota / period;

    /*
     Here can be implemented: hypervisor agnostic real-time schedulability tests, etc.
    */

    //writeln!(logfile, "Calling CPU config")?;                         //DEBUG
    //let start_time = Instant::now();                                                    //TIME
    let _ = cpu::cpuconf(fc, &mut c, &quota, &period, &cpus);
    let _ = append_message_with_time(&format!("Finished cpu config")); //TIME

    //This region of code could be extended through code to retrieve other specific Docker's flags which set MEM limitations

    // Extract values from the JSON structure
    //In the json structure only limit is created by kubernetes memory reservation doesn't exist, (but it cluod be specified in other way??), anyway we need to cuild the lv for the vm
    let st_req = fc.jsonconfig["linux"]["resources"]["memory"]["reservation"] //Domain memory in MB, (--memory-reservation="")
    .as_u64() // Assuming memory values are in unsigned integers
    .unwrap_or(512); // Set default value to 512 MB if the value is missing

    let mem_request = fc.jsonconfig["linux"]["resources"]["memory"]["limit"] //Maximum domain memory in MB, (-m, --memory="")
        .as_u64() // Assuming memory values are in unsigned integers
        .unwrap_or(512); // Set default value to 2G if the value is missing
                                //writeln!(logfile, "Mem_Requested is: {}", mem_request)?;       //DEBUG

    //let start_time = Instant::now();                                                    //TIME
    
    //Pass everything to memconfig
    let _ = append_message_with_time(&format!("starting mem config")); //TIME
    let _ = append_message_with_time(&mem_request.to_string()); //TIME
    let _ = mem::memconf(&mut c,&st_req, &mem_request,LVM_GROUP_NAME);
    let _ = append_message_with_time(&format!("finished mem config")); //TIME
    //let _ = append_message_with_time(&format!("Time elapsed in mem config is: {:?}", start_time.elapsed())); //TIME


    //-------------------------------------------------------------------------------------
    //In xen physical device are managed by dom0
    //-------------------------------------------------------------------------------------
    //writeln!(logfile, "Memoryend - devconfig start")?;       //DEBUG
    //let start_time = Instant::now();                                                    //TIME
    //let _ = append_message_with_time(&format!("starting dev cfg")); //TIME
    //let _ = device::devconfig(&mut c);
    //let _ = append_message_with_time(&format!("finished dev cfg")); //TIME
    //let _ = append_message_with_time(&format!("Time elapsed in dev config is: {:?}", start_time.elapsed())); //TIME

    //let start_time = Instant::now();                                                    //TIME
    //let _ = append_message_with_time(&format!("Time elapsed in boot config is: {:?}", start_time.elapsed())); //TIME


    let _ = append_message_with_time(&format!("network config start")); //TIME
    let _ = network::netconfig(&mut c);
    let _ = append_message_with_time(&format!("network config end")); //TIME


    // Guest console is allocated when -t flag is provided
    // useful for Hypervisor like XEN or BAO which give the possibility
    // to start Guest with fully fledged OS
    // Jailhouse gives only the option to start non-root linux cell but the user can connect to it
    // only through ssh



    //------------------------------------------------------------------------------------
    //IDK if is needed for XEN, but il'll leave it 'cause i think it's not a problem
    //------------------------------------------------------------------------------------
    // If -t flag was specified, call COMMUNICATION backend for further processing
    // E.G. allocate terminal or ssh shell
    //writeln!(logfile, "Before guestconsole is empty")?; //DEBUG
    if !fc.guestconsole.is_empty() {
        let mut file = fs::File::create(format!("{}/console", fc.crundir))
            .expect("Failed to create console file");
        writeln!(file, "{}", fc.guestconsole).expect("Failed to write console file");
    }
    //let start_time = Instant::now();                                                    //TIME

    let _ = append_message_with_time(&format!("console created")); //TIME

    //------------------------------------------------------------------------------------
    //The comuniccation between the dooms, it should be possible simply by the virtaual networw interface
    //------------------------------------------------------------------------------------
    //let _ = communication::communicationconfig(&mut c);
    
    let _ = confighelperend(fc, &mut c, &config);
    
    let duration = start_time.elapsed().unwrap();
    let _ = append_time(&duration.as_micros().to_string());
    return Ok(config);
}

fn confighelperstart(
    fc: &f2b::FrontendConfig,
    c: &mut Backendconfig,
    _ic: &f2b::ImageConfig,
) -> Result<(), Box<dyn Error>> {
    // Write the generic header to the conf file
    //let mut file = File::create(conf)?;
    
    c.conf = format!("
#---------------------------------------------------------------
#Configuration file for container with id : {} 
#---------------------------------------------------------------

name = \"{}\" \n\n"

    ,fc.containerid,fc.containerid);

    return Ok(());
}

fn confighelperend(
    fc: &f2b::FrontendConfig,
    c: &mut Backendconfig,
    _ic: &f2b::ImageConfig,
) -> Result<(), Box<dyn Error>> {
    //writeln!(logfile, "after regex of helperend")?; //DEBUG
    let _ = append_message_with_time(&format!("attepting to write config")); //TIME
    let _ = append_message_with_time(&format!("Writing to {}", &c.conffile));
    //create and write the file
    std::fs::write(&c.conffile, &c.conf)?;
    let _ = append_message_with_time(&format!("\n{}\n",&c.conf)); //TIME

    let _ = append_message_with_time(&format!("config wrote")); //TIME


    return Ok(());
}

#[allow(dead_code)]
fn append_message_with_time(message: &str) -> Result<(), Box<dyn Error>> {  //TIME

    // Open the file in append mode, create it if it doesn't exist
    let mut timefile = OpenOptions::new()
    .create(true)
    .append(true)
    .open("/usr/share/runPHI/times_file.txt")?;
    
    // Write the message and current time to the file, separated by an equal sign
    writeln!(timefile, "{}", message)?;
    
    Ok(())
}

fn append_time(message: &str) -> Result<(), Box<dyn Error>> {  //TIME

    // Open the file in append mode, create it if it doesn't exist
    let mut timefile = OpenOptions::new()
    .create(true)
    .append(true)
    .open("/usr/share/runPHI/times.txt")?;
    
    // Write the message and current time to the file, separated by an equal sign
    writeln!(timefile, "{}", message)?;
    
    Ok(())
}