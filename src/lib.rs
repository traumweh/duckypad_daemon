pub mod hid;
pub mod x11;

use hidapi::HidApi;
use serde_json::Value;
use std::{fs::File, io::prelude::Write, path::PathBuf, process::Command};
use x11::{active_window, ActiveWindow, RustConnection, System};

fn create_default_config(path: &PathBuf) {
    eprintln!("Creating default config, because file doesn't exist");
    let mut file = File::create(path)
        .unwrap_or_else(|error| panic!("Couldn't create config file:\n{}", error));
    file.write_all(b"{\"autoswitch_enabled\": false, \"rules_list\": []}")
        .expect("Couldn't write to config file!");
}

/// Returns a PathBuf for the config file path and creates a default config if
/// no config file exists yet.
///
/// Default config path is XDG_CONFIG_DIR/duckypad_autoswitcher/config.txt
///
/// # Arguments
///
/// * `path` - Override directory in which the config file exists or should be created.
///
/// # Panics
///
/// The function will panic if `path`/config.txt isn't a file or couldn't be created.
///
/// # Examples
///
/// ```
/// let config = config_file(None);
/// ```
pub fn config_file(path: Option<PathBuf>) -> PathBuf {
    if let Some(config) = path {
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

/// Returns a serde Value object that represents the current contents of the
/// configuration file.
///
/// # Arguments
///
/// * `path` - Path to the config file
///
/// # Examples
///
/// ```
/// let config = read_config(config_file(None));
/// ```
pub fn read_config(path: &PathBuf) -> Value {
    let file =
        File::open(&path).unwrap_or_else(|error| panic!("Error reading config file:\n{}", error));
    let reader = std::io::BufReader::new(file);
    let json: Value = serde_json::from_reader(reader)
        .unwrap_or_else(|error| panic!("Error parsing config file as json:\n{}", error));

    json
}

/// Switches to the next profile if it is different from the previous one and
/// returns it.
///
/// # Arguments
///
/// * `api` - valid api connection
/// * `device` - connected duckypad hid device
/// * `profile` - id of the profile on the duckypad (1 <= id <= 31)
/// * `callback` - optional command to spawn
pub fn switch_profile(
    api: &HidApi,
    config: &Value,
    con: &RustConnection,
    screen: usize,
    sys: &mut System,
    prev_profile: Option<u8>,
    callback: &mut Option<Command>,
) -> Option<u8> {
    let window = active_window(con, screen, sys);
    let profile = next_profile(&config, &window);

    if let Some(profile) = profile {
        if prev_profile.is_none() || profile != prev_profile.unwrap() {
            if let Ok(duckypad) = hid::init(&api) {
                if let Ok(_) = goto_profile(&duckypad, profile) {
                    if let Some(callback) = callback {
                        run_callback(callback, profile, window);
                    }
                    return Some(profile);
                }
            }
        }
    }

    prev_profile
}

/// Runs a callback executable if `callback.is_some()` by spawning a child with
/// the following arguments:
/// ```
/// -p <PROFILE> [-c <CMD>] [-w <WM_CLASS>] [-n <WM_NAME>]
/// ```
/// The placeholder `"N/A"` will be supplied if either of `<cmd>`, `<wm_class>`
/// or `<wm_name>` couldn't be determined.
///
/// # Arguments
///
/// * `callback` - optional callback script to run on change
/// * `profile` - id of the profile on the duckypad (1 <= id <= 31)
/// * `window` - information about the active window
pub fn run_callback(callback: &mut Command, profile: u8, window: ActiveWindow) -> () {
    let mut callback = callback.arg("-p").arg(profile.to_string());

    if let Some(cmd) = window.cmd {
        callback = callback.arg("-c").arg(cmd);
    }
    if let Some(wm_class) = window.wm_class {
        callback = callback.arg("-w").arg(wm_class);
    }
    if let Some(wm_name) = window.wm_name {
        callback = callback.arg("-n").arg(wm_name);
    }

    if let Err(err) = callback.spawn() {
        eprintln!("Failed to run callback: {}", err);
    }
}

/// Returns a Result that is either the unit () or a HidError.
///
/// # Arguments
///
/// * `device` - connected duckypad hid device
/// * `profile` - id of the profile on the duckypad (1 <= id <= 31)
pub fn goto_profile(device: &hidapi::HidDevice, profile: u8) -> Result<(), hidapi::HidError> {
    assert!(profile >= 1 && profile <= 31); // duckyPad limits

    println!("Switching to profile {}", profile);
    let mut buf = [0x00; hid::PC_TO_DUCKYPAD_HID_BUF_SIZE];
    buf[0] = 0x05;
    buf[2] = 0x01;
    buf[3] = profile;

    let _ = hid::write(device, buf)?;
    Ok(())
}

/// Returns the id of the profile to switch to based on the active X11 window
/// and the config entries.
///
/// # Arguments
///
/// * `config` - serde Value of the current configuration
/// * `window` - information about the active window
pub fn next_profile(config: &Value, window: &ActiveWindow) -> Option<u8> {
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

            if let Some(app_name) = &window.cmd {
                correct_app_name |= app_name.contains(rule_app_name);
            }

            // NEW *OPTIONAL* CONFIG FIELD
            let rule_window_class = item.get("window_class");
            let mut correct_window_class = rule_window_class.is_none();

            if let Some(rule_window_class) = rule_window_class {
                let rule_window_class = rule_window_class.as_str().expect(ERR_STR);
                correct_window_class |= rule_window_class.len() == 0;

                if let Some(window_class) = &window.wm_class {
                    correct_window_class |= window_class.contains(rule_window_class);
                }
            }

            let rule_window_title = item
                .get("window_title")
                .expect(ERR_STR)
                .as_str()
                .expect(ERR_STR);

            let mut correct_window_title = rule_window_title.len() == 0;

            if let Some(window_title) = &window.wm_name {
                correct_window_title |= window_title.contains(rule_window_title);
            }

            if correct_app_name && correct_window_class && correct_window_title {
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
