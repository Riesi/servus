use serde::{Deserialize, Serialize};
use std::error;

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigStruct {
    pub token: String,
    pub url: String,
}

impl Default for ConfigStruct {
    fn default() -> Self {
        ConfigStruct {
            token: "LONG_LIVED_TOKEN".to_string(),
            url: "http://my-fancy-server-address.local/dashboard?kiosk".to_string(),
        }
    }
}

pub fn write_example_config() {
    let f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open("config.yml")
        .expect("Couldn't open file.");
    serde_yaml::to_writer(f, &ConfigStruct::default()).unwrap();
    println!("Failed to read config file!\nExample file written instead.");
}

pub fn read_config() -> Result<ConfigStruct, Box<dyn error::Error>> {
    let f = std::fs::File::open("./config.yml")?;
    Ok::<ConfigStruct, _>(serde_yaml::from_reader(f)?)
}
