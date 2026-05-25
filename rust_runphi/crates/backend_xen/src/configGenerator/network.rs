use std::error::Error;

use crate::configGenerator;

pub fn netconfig(_c: &mut configGenerator::Backendconfig) -> Result<(), Box<dyn Error>> {
    // Insert line into the config file
    // In arm we don't have a bridge
    //let linetoinsert = "vif = ['bridge=xenbr0'] \n"; //Commented because raises an error if the bridge does not exist

    //c.conf.push_str(&format!("\n{}\n", linetoinsert));

    Ok(())
}
