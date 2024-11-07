use std::error::Error;

use crate::configGenerator;

pub fn netconfig(c: &mut configGenerator::Backendconfig) -> Result<(), Box<dyn Error>> {
    // Insert line into the config file
    let linetoinsert = "vif = ['bridge=xenbr0'] \n";

    c.conf.push_str(&format!("\n{}\n", linetoinsert));

    Ok(())
}
