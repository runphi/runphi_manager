//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//*********************************************

use regex::Regex;
use std::error::Error;

use crate::configGenerator;
//use f2b;

pub fn communicationconfig(c: &mut configGenerator::Backendconfig) -> Result<(), Box<dyn Error>> {
    // Insert line into the config file
    let pattern = r"cell = \{";
    //let linetoinsert = "\t.console = {.address = 0x09000000,.type = JAILHOUSE_CON_TYPE_PL011,.flags = JAILHOUSE_CON_ACCESS_MMIO | JAILHOUSE_CON_REGDIST_4,	},";
    let linetoinsert = "\
    .console = {\n\
        \t.address = 0x09000000,\n\
        \t.type = JAILHOUSE_CON_TYPE_PL011,\n\
        \t.flags = JAILHOUSE_CON_ACCESS_MMIO |\n\
        \t\t JAILHOUSE_CON_REGDIST_4,\n\
    },";
    // Compile a regular expression to match the pattern and insert the cpus
    let re = Regex::new(&pattern)?;
    if let Some(pos) = re.find(&c.conf) {
        c.conf
            .insert_str(pos.end(), &format!("\n{}\n", linetoinsert));
    } else {
        return Err("\"cell = {\" not found".into());
    }

    Ok(())
}
