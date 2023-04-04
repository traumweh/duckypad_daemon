pub mod hid;

use active_win_pos_rs::{get_active_window, ActiveWindow, WindowPosition};
use hidapi::HidApi;
use serde_json::Value;
use std::{
    fs::File,
    io::prelude::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

pub mod enums {
    pub enum LinuxServer {
        WAYLAND(std::path::PathBuf),
        XORG,
    }

    pub enum OSIdent {
        MACOS,
        WINDOWS,
        LINUX(LinuxServer),
        UNSUPPORTED,
    }
}

fn create_default_config(path: &PathBuf) {
    eprintln!("Creating default config, because file doesn't exist");
    let mut file = File::create(path)
        .unwrap_or_else(|error| panic!("Couldn't create config file:\n{}", error));
    file.write_all(b"[]")
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

    const ERR_STR: &str = "Failed to determine config location";
    let mut config = dirs_next::config_dir().expect(ERR_STR);
    config.push("duckypad_daemon/config.json");

    if !config.exists() {
        let parent = config.parent().expect(ERR_STR);

        if !parent.exists() {
            std::fs::create_dir_all(parent).expect(ERR_STR);
        }

        create_default_config(&config);
    }

    config
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
/// * `config` - current configuration
/// * `prev_profile` - id of the profile on the duckypad (1 <= id <= 31)
/// * `callback` - optional command to spawn
/// * `os` - enum value of the running operating system
pub fn switch_profile(
    api: &HidApi,
    config: &Value,
    prev_profile: Option<u8>,
    callback: &mut Option<Command>,
    os: &enums::OSIdent,
) -> Option<u8> {
    let window = match os {
        enums::OSIdent::UNSUPPORTED => panic!("You are running an unsupported OS!\n"),
        enums::OSIdent::LINUX(enums::LinuxServer::WAYLAND(script)) => wayland_active_window(script),
        _ => get_active_window(),
    };

    if let Ok(window) = window {
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
    }

    prev_profile
}

fn wayland_active_window(script: &PathBuf) -> Result<ActiveWindow, ()> {
    const ERR_STR: &str = "Malformed output from wayland-script!\n";
    let output = Command::new(script).stdout(Stdio::piped()).output();

    if let Ok(output) = output {
        let raw = String::from_utf8(output.stdout).expect(ERR_STR);
        let json: Value = serde_json::from_str(&raw).expect(ERR_STR);
        let title = json
            .get("title")
            .expect(ERR_STR)
            .as_str()
            .expect(ERR_STR)
            .to_string();
        let process_name = json
            .get("process_name")
            .expect(ERR_STR)
            .as_str()
            .expect(ERR_STR)
            .to_string();
        let window_id = json
            .get("window_id")
            .unwrap_or(&Value::String("".to_string()))
            .as_str()
            .expect(ERR_STR)
            .to_string();
        let process_id = json
            .get("process_id")
            .expect(ERR_STR)
            .as_u64()
            .expect(ERR_STR);
        let position = if let Some(pos) = json.get("position") {
            let pos = pos.as_object().expect(ERR_STR);
            let x = pos.get("x").expect(ERR_STR).as_f64().expect(ERR_STR);
            let y = pos.get("y").expect(ERR_STR).as_f64().expect(ERR_STR);
            let w = pos.get("w").expect(ERR_STR).as_f64().expect(ERR_STR);
            let h = pos.get("h").expect(ERR_STR).as_f64().expect(ERR_STR);
            WindowPosition::new(x, y, w, h)
        } else {
            WindowPosition::new(0.0, 0.0, 0.0, 0.0)
        };
        let active_window: ActiveWindow = ActiveWindow {
            title,
            process_name,
            window_id,
            process_id,
            position,
        };
        return Ok(active_window);
    }

    Err(())
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

    if window.process_name != "" {
        callback = callback.arg("-w").arg(window.process_name);
    }
    if window.title != "" {
        callback = callback.arg("-n").arg(window.title);
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

    let config = config.as_array().expect(ERR_STR);

    for item in config.iter() {
        let item = item.as_object().expect(ERR_STR);
        if item
            .get("enabled")
            .expect(ERR_STR)
            .as_bool()
            .expect(ERR_STR)
        {
            let process_name = item
                .get("process_name")
                .expect(ERR_STR)
                .as_str()
                .expect(ERR_STR);
            let title = item.get("title").expect(ERR_STR).as_str().expect(ERR_STR);

            if (process_name.len() == 0 || window.process_name.contains(process_name))
                && (title.len() == 0 || window.title.contains(title))
            {
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
