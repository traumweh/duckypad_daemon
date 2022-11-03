pub mod hid;
pub mod x11;

use serde_json::Value;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

fn create_default_config(path: &PathBuf) {
    eprintln!("Creating default config, because file doesn't exist");
    let mut file = File::create(path).expect("Couldn't create config file!");
    file.write_all(b"{\"autoswitch_enabled\": false, \"rules_list\": []}")
        .expect("Couldn't write to config file!");
}

pub fn config_file(path: &Option<String>) -> std::path::PathBuf {
    if let Some(config_path) = path {
        let config = PathBuf::from(config_path);

        if !config.exists() {
            create_default_config(&config);
        }

        if !config.is_file() {
            panic!("Supplied config-path isn't a file!");
        }

        return config;
    }

    let xdg_dirs = xdg::BaseDirectories::with_prefix("duckypad_autoswitcher")
        .expect("Failed to determine config location");
    let config = xdg_dirs.find_data_file("config.txt");

    if config.is_none() {
        let config_path = xdg_dirs
            .place_data_file("config.txt")
            .expect("Couldn't create config directory!");
        create_default_config(&config_path);
        return config_path;
    }

    config.unwrap()
}

pub fn read_config(path: &PathBuf) -> Value {
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|_| panic!("Error reading file: '{}'", path.display()));
    let reader = std::io::BufReader::new(file);
    let json: Value = serde_json::from_reader(reader)
        .unwrap_or_else(|_| panic!("Error parsing file as json: '{}'", path.display()));

    json
}

pub fn goto_profile(device: &hidapi::HidDevice, profile: u8) -> Result<(), hidapi::HidError> {
    assert!(profile >= 1 && profile <= 31); // duckyPad limits

    println!("Switching to profile {}", profile);
    let mut buf = [0; hid::PC_TO_DUCKYPAD_HID_BUF_SIZE];
    buf[0] = 5;
    buf[2] = 1;
    buf[3] = profile;

    let _ = hid::write(device, buf)?;
    Ok(())
}

pub fn next_profile(config: &Value) -> Option<u8> {
    let (app_name, window_title) = x11::active_window();
    const ERR_STR: &str = "Malformed config JSON!";

    let config = config.as_object().expect(ERR_STR);
    let rules = config
        .get("rules_list")
        .expect(ERR_STR)
        .as_array()
        .expect(ERR_STR);

    for item in rules.iter() {
        let item = item.as_object().expect(ERR_STR);
        if item
            .get("enabled")
            .expect(ERR_STR)
            .as_bool()
            .expect(ERR_STR)
        {
            let rule_app_name = item
                .get("app_name")
                .expect(ERR_STR)
                .as_str()
                .expect(ERR_STR);
            let mut correct_app_name = rule_app_name.len() == 0;

            if let Some(app_name) = &app_name {
                correct_app_name |= app_name.contains(
                    item.get("app_name")
                        .expect(ERR_STR)
                        .as_str()
                        .expect(ERR_STR),
                );
            }

            let rule_window_title = item
                .get("window_title")
                .expect(ERR_STR)
                .as_str()
                .expect(ERR_STR);

            let mut correct_window_title = rule_window_title.len() == 0;

            if let Some(window_title) = &window_title {
                correct_window_title = window_title.contains(
                    item.get("window_title")
                        .expect(ERR_STR)
                        .as_str()
                        .expect(ERR_STR),
                );
            }

            if correct_app_name && correct_window_title {
                let profile = item
                    .get("switch_to")
                    .expect(ERR_STR)
                    .as_u64()
                    .expect(ERR_STR);

                return Some(u8::try_from(profile).expect(ERR_STR));
            }
        }
    }

    None
}
