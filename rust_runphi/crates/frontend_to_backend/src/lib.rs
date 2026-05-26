//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

pub mod paths;

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
    pub crundir: PathBuf,
    pub containerid: String,
    pub bundle: PathBuf,
    pub mountpoint: PathBuf,
    pub guestconsole: PathBuf,
    pub pidfile: PathBuf,
}
impl Default for FrontendConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl FrontendConfig {
    pub fn new() -> Self {
        Self {
            jsonconfig: serde_json::Value::Null,
            crundir: PathBuf::new(),
            containerid: String::new(),
            bundle: PathBuf::new(),
            mountpoint: PathBuf::new(),
            guestconsole: PathBuf::new(),
            pidfile: PathBuf::new(),
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
    // OSvar stores information in a file /OS to identify the OS to load.
    // If OSvar is "linux", the OS file contains the image of the linux kernel.
    // If OSvar is "zephyr", zephyr is loaded; zephyr is a de-facto bare-metal
    // inmate and the os_var is only needed at the moment to mock up the .cell
    // file, but it is useful to keep the OS information around for the future.
    // When available, this variable could also identify another bare-metal
    // runtime like a WASM OS.
    //TODO: drop the zephyr os_var once the .cell mockup no longer needs it.
    pub os_var: String,
    #[serde(default)]
    // When available, a custom kernel and ramkdisk shipped in the container can be specified. In this case the
    // application decides to bring its own kernel (platformm/board-dependent)
    pub kernel: String,
    #[serde(default)]
    pub ramdisk: String,
    // The inmate variable represents the file to be loaded containing the bare metal code or the
    // app with the libOS
    #[serde(default)]
    pub inmate: String,
    // The dtb is only for linux arm64, borderline case
    #[serde(default)]
    pub dtb: String,
    // Same here, the initrd is an alternative to ramdisk, depending on the arch
    #[serde(default)]
    pub initrd: String,
    #[serde(default)]
    pub netconf: String,
    // The starting_vaddress variable specifies the virtual address that the binary in inmate is
    // expecting to start. This determines how to remap the memory in the MMU when available, or
    // decides the placement when MMU not avaialble
    #[serde(default)]
    pub starting_vaddress: String,
    // This lines are needed to include the "net" and "rpu_req" field
    #[serde(default)]
    pub net: String,
    #[serde(default)]
    pub rpu_req: bool
    // TODO: handle default or missing values in a decent way
}
impl ImageConfig {
    pub fn get_from_file(mountpoint: &Path) -> Result<Self, Box<dyn Error>> {
        // parsing configuration variables from the file
        //TODO: here is the case to parse also a node default used in the case the container does not specify this
        let path = mountpoint.join(paths::BOOT_CONFIG_REL);
        let json_str = fs::read_to_string(&path)
            .map_err(|e| format!("cannot read runPHI boot config {}: {}", path.display(), e))?;
        let mut config: ImageConfig = serde_json::from_str(&json_str)
            .map_err(|e| format!("malformed runPHI boot config {}: {}", path.display(), e))?;
        // ic.inmate stays a String because the surrounding code mixes "path
        // under rootfs" with text rendered into the cell config; a PathBuf
        // here would just push string conversions outward.
        if !config.inmate.is_empty() {
            config.inmate = format!("{}{}", mountpoint.display(), config.inmate)
                .trim()
                .to_string();
        } else {
            config.inmate = mountpoint
                .join(paths::BOOT_INMATE_DEFAULT_REL)
                .display()
                .to_string();
        }
        Ok(config)
    }
}
