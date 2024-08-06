//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (f.boccola@studenti.unina.it)
//*********************************************

use crate::configGenerator;
use regex::Regex;
use std::collections::HashSet;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
//use f2b;

const WORKPATH: &str = "/usr/share/runPHI";
const PCI_IVSHMEM_ID_FILE: &str = "pci_ivshmem_id.txt";

fn read_bdf() -> io::Result<Option<u8>> {
    let path = Path::new(WORKPATH).join(PCI_IVSHMEM_ID_FILE);
    if path.exists() {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let used_ids: HashSet<u8> = contents
            .lines()
            .filter_map(|line| line.trim().parse().ok())
            .collect();

        //We have 1..=2 as bdfs since 0 is now always assigned to the ivshmem with 3 peers (root and 2 non-root)
        //Also every non-root cell must have the ivshmem with 3 peers, so other two cells with net maximum
        for id in 1..=2 {
            if !used_ids.contains(&id) {
                return Ok(Some(id));
            }
        }
        Ok(None)
    } else {
        Ok(Some(1))
    }
}

fn write_bdf(bdf: u8) -> io::Result<()> {
    let path = Path::new(WORKPATH).join(PCI_IVSHMEM_ID_FILE);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", bdf)
}

pub fn devconfig(c: &mut configGenerator::Backendconfig) -> Result<(), Box<dyn Error>> {
    // Insert line into the config file
    let pattern = r"struct jailhouse_cell_desc cell;\n.*\n.*";
    // Check the value of c.net and set linetoinsert accordingly
    let linetoinsert = if c.net == "none" {
        "\tstruct jailhouse_irqchip irqchips[1];\n\tstruct jailhouse_pci_device pci_devices[0];"
    } else {
        "\tstruct jailhouse_irqchip irqchips[1];\n\tstruct jailhouse_pci_device pci_devices[2];"
    };

    // Compile a regular expression to match the pattern and insert the cpus
    let re = Regex::new(&pattern)?;
    if let Some(pos) = re.find(&c.conf) {
        c.conf
            .insert_str(pos.end(), &format!("\n{}\n", linetoinsert));
    } else {
        return Err("\"struct jailhouse_cell_desc cell\" not found".into());
    }

    if c.net == "none" {
        let pci_device_config = format!(
            "\t.irqchips = {{
                /* GIC */ {{
                    .address = 0x08000000,
                    .pin_base = 32,
                    .pin_bitmap = {{
                        1 << (33 - 32),
                        0,
                        0,
                        0 
                    }},
                }},
            }},
        
            .pci_devices = {{
            }},"
        );
        // Append pci_device_config to the configuration
        c.conf.push_str(&pci_device_config);
    } else {
        // Read the current BDF value
        let bdf = match read_bdf()? {
            Some(bdf) => bdf,
            None => return Err("No free PCI devices available".into()),
        };

        // The .irqchips field sets the 33 bit (for the uart)
        // and the bit 140 for ivshmem 3 peers (unused) and 140+bdf (141 - 142 - 143) for the 3 ivshmem with 2 peers

        // Update configuration with dynamic BDF value
        let pci_device_config = format!(
            "\t.irqchips = {{
                /* GIC */ {{
                    .address = 0x08000000,
                    .pin_base = 32,
                    .pin_bitmap = {{
                        1 << (33 - 32),
                        0,
                        0,
                        (1 << (140 - 128)) | (1 << ({} - 128)) 
                    }},
                }},
            }},
        
            .pci_devices = {{
                {{ /* IVSHMEM 00:00.0 (demo) */
                    .type = JAILHOUSE_PCI_TYPE_IVSHMEM,
                    .domain = 1,
                    .bdf = 0 << 3,
                    .bar_mask = JAILHOUSE_IVSHMEM_BAR_MASK_INTX,
                    .shmem_regions_start = 0,
                    .shmem_dev_id = {},
                    .shmem_peers = 3,
                    .shmem_protocol = JAILHOUSE_SHMEM_PROTO_UNDEFINED,
                }},
                {{ /* IVSHMEM 00:{:02X}.0 (networking) */
                    .type = JAILHOUSE_PCI_TYPE_IVSHMEM,
                    .domain = 1,
                    .bdf = {} << 3,
                    .bar_mask = JAILHOUSE_IVSHMEM_BAR_MASK_INTX,
                    .shmem_regions_start = 5,
                    .shmem_dev_id = 1,
                    .shmem_peers = 2,
                    .shmem_protocol = JAILHOUSE_SHMEM_PROTO_VETH,
                }},
            }},",
            140 + bdf,
            bdf,
            bdf,
            bdf
        );

        // Append pci_device_config to the configuration
        c.conf.push_str(&pci_device_config);

        // Write the new BDF value to mark it as used
        write_bdf(bdf)?;
    };

    Ok(())
}
