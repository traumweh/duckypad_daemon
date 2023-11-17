#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate)]

pub mod hid;

use active_win_pos_rs::{get_active_window, ActiveWindow, WindowPosition};
use hidapi::HidApi;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::File,
    io::prelude::Write,
    path::PathBuf,
    process::{Command, Stdio},
};
use sysinfo::{Pid, ProcessExt, ProcessRefreshKind, System, SystemExt};

pub mod enums {
    pub enum LinuxServer {
        WAYLAND(std::path::PathBuf),
        XORG,
    }

    pub enum OSIdent {
        MACOS,
        WINDOWS,
        LINUX(LinuxServer),
        UNSUPPORTED(std::path::PathBuf),
    }
}

#[derive(Serialize, Deserialize)]
pub struct Rules {
    app_name: String,
    process_name: Option<String>,
    #[serde(alias = "title")]
    window_title: String,
    enabled: bool,
    switch_to: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    autoswitch_enabled: Option<bool>,
    rules_list: Vec<Rules>,
}

fn create_default_config(path: &PathBuf) {
    eprintln!("Creating default config, because file doesn't exist");
    let mut file =
        File::create(path).unwrap_or_else(|error| panic!("Couldn't create config file:\n{error}"));

    file.write_all(
        serde_json::to_string(&Config {
            autoswitch_enabled: Some(false),
            rules_list: vec![],
        })
        .expect("Failed to serialize default config.")
        .as_bytes(),
    )
    .expect("Couldn't write to config file!");
}

/// Returns a `PathBuf` for the config file path and creates a default config if
/// no config file exists yet.
///
/// Default config path is `XDG_CONFIG_DIR/duckypad_autoswitcher/config.txt`
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

        assert!(config.is_file(), "Supplied config-path isn't a file!");
        return config;
    }

    let mut config = dirs_next::config_dir()
        .expect("Unable to determine platform specific default for config files!");
    config.push("duckypad_daemon/config.json");

    if !config.exists() {
        let parent = config
            .parent()
            .expect("Unable to get parent path of config directory!");

        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .unwrap_or_else(|err| panic!("Unable to create config directory: {err}"));
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
///
/// # Panics
///
/// This function will panic either if the config file at `path` cannot be read
/// from or if it cannot be parsed as a JSON.
pub fn read_config(path: &PathBuf) -> Config {
    let file =
        File::open(path).unwrap_or_else(|error| panic!("Error reading config file:\n{error}"));
    let reader = std::io::BufReader::new(file);
    let config: Config = serde_json::from_reader(reader)
        .unwrap_or_else(|error| panic!("Error parsing and deserialize config file:\n{error}"));

    config
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
    sys: &mut Option<System>,
    config: &Config,
    prev_profile: Option<u32>,
    callback: &mut Option<Command>,
    os: &enums::OSIdent,
) -> Option<u32> {
    let window = match os {
        enums::OSIdent::UNSUPPORTED(script)
        | enums::OSIdent::LINUX(enums::LinuxServer::WAYLAND(script)) => {
            custom_active_window(script)
        }
        _ => get_active_window(),
    };

    if let Ok(window) = window {
        #[allow(clippy::cast_possible_truncation)]
        let app_name = get_app_name(sys, Pid::from(window.process_id as usize))
            .unwrap_or("unknown".to_string());

        if let Some(profile) = next_profile(config, &window, &app_name) {
            if match prev_profile {
                Some(prev_profile) => profile != prev_profile,
                None => true,
            } {
                if let Ok(duckypad) = hid::init(api) {
                    if goto_profile(&duckypad, profile).is_ok() {
                        if let Some(callback) = callback {
                            run_callback(callback, profile, window, &app_name);
                        }
                        return Some(profile);
                    }
                }
            }
        }
    }

    prev_profile
}

/// Gets information about the active window by calling a script that is passed
/// via the --window-script,-s command-line option.
/// The script must output a JSON object with the following structure (item
/// order doesn't matter):
/// ```json
/// {
///     "title": str,
///     "process_name": str
/// }
/// ```
/// It can optionally contain more information that might have future purpose
/// but will be ignored for now. A full JSON object would look like this:
/// ```json
/// {
///     "title": str,
///     "process_name": str,
///     "process_id": u64,
///     "window_id": str,
///     "position":{
///          "x": f64,
///          "y": f64,
///          "w": f64,
///          "h": f64
///     }
/// }
/// ```
///
/// # Arguments
///
/// * `script` - path of executable for custom window information
fn custom_active_window(script: &PathBuf) -> Result<ActiveWindow, ()> {
    let output = Command::new(script).stdout(Stdio::piped()).output();

    if let Ok(output) = output {
        let raw =
            String::from_utf8(output.stdout).expect("Window script output needs to be valid utf8!");
        let json: Value =
            serde_json::from_str(&raw).expect("Window script needs to be a JSON object!");
        let title = json
            .get("title")
            .expect("Window script output field \"title\" is missing!")
            .as_str()
            .expect("Window script output field \"title\" needs to be a string!")
            .to_string();
        let process_name = json
            .get("process_name")
            .expect("Window script output field \"process_name\" is missing!")
            .as_str()
            .expect("Window script output field \"process_name\" needs to be a string!")
            .to_string();
        let window_id = json
            .get("window_id")
            .unwrap_or(&Value::String(String::new()))
            .as_str()
            .expect("Window script output field \"window_id\" needs to be a string!")
            .to_string();
        let process_id = json
            .get("process_id")
            .unwrap_or(&Value::Number(serde_json::Number::from_f64(0.0).unwrap()))
            .as_u64()
            .expect("Window script output field \"process_id\" needs to be an unsigned int (u64)!");
        let position = if let Some(pos) = json.get("position") {
            let pos = pos
                .as_object()
                .expect("Window script output field \"position\" needs to be a JSON array!");
            let x = pos
                .get("x")
                .expect("Window script output field \"x\" is missing!")
                .as_f64()
                .expect("Window script output field \"x\" needs to be a float (f64)!");
            let y = pos
                .get("y")
                .expect("Window script output field \"y\" is missing!")
                .as_f64()
                .expect("Window script output field \"y\" needs to be a float (f64)!");
            let w = pos
                .get("w")
                .expect("Window script output field \"w\" is missing!")
                .as_f64()
                .expect("Window script output field \"w\" needs to be a float (f64)!");
            let h = pos
                .get("h")
                .expect("Window script output field \"h\" is missing!")
                .as_f64()
                .expect("Window script output field \"h\" needs to be a float (f64)!");
            WindowPosition::new(x, y, w, h)
        } else {
            WindowPosition::new(0.0, 0.0, 0.0, 0.0)
        };
        let active_window: ActiveWindow = ActiveWindow {
            title,
            process_path: PathBuf::new(), // TODO: Ignore path for now
            app_name: process_name,
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
/// -p <PROFILE> [-a <APP_NAME>] [-t <TITLE>] [-n <PROCESS_NAME>]
/// ```
///
/// # Arguments
///
/// * `callback` - optional callback script to run on change
/// * `profile` - id of the profile on the duckypad (1 <= id <= 31)
/// * `window` - information about the active window
pub fn run_callback(callback: &mut Command, profile: u32, window: ActiveWindow, app_name: &String) {
    let mut callback = callback.arg("-p").arg(profile.to_string());

    if !app_name.is_empty() {
        callback = callback.arg("-a").arg(app_name);
    }
    if !window.title.is_empty() {
        callback = callback.arg("-t").arg(window.title);
    }
    if !window.app_name.is_empty() {
        callback = callback.arg("-n").arg(window.app_name);
    }

    match callback.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _: Result<_, _> = child.wait();
            });
        }
        Err(err) => {
            eprintln!("Failed to run callback: {err}");
        }
    };
}

fn get_app_name(sys: &mut Option<System>, pid: Pid) -> Option<String> {
    if let Some(sys) = sys {
        sys.refresh_process_specifics(pid, ProcessRefreshKind::new());
        let process = sys.process(pid);

        if let Some(process) = process {
            return Some(process.name().to_string());
        }
    }

    None
}

/// Switch to the `profile` by sending a HID message to the duckypad.
///
/// # Arguments
///
/// * `device` - connected duckypad hid device
/// * `profile` - id of the profile on the duckypad (1 <= id <= 31)
///
/// # Errors
///
/// Will return `HidError` if writing to or the follow-up reading from the
/// duckypad `HidDevice` fails.
///
/// # Panics
///
/// The function will panic if `profile` is not a value in `(1..=31)`.
pub fn goto_profile(device: &hidapi::HidDevice, profile: u32) -> Result<(), hidapi::HidError> {
    println!("Switching to profile {profile}");
    let mut buf = [0x00; hid::PC_TO_DUCKYPAD_HID_BUF_SIZE];
    let profile_buf = profile.to_le_bytes();

    buf[0] = 0x05;
    buf[2] = 0x01;

    for (i, p) in profile_buf.iter().enumerate() {
        buf[3 + i] = *p;
    }

    hid::write(device, buf)?;
    Ok(())
}

/// Returns the id of the profile to switch to based on the active X11 window
/// and the config entries.
///
/// # Arguments
///
/// * `config` - serde Value of the current configuration
/// * `window` - information about the active window
pub fn next_profile(config: &Config, window: &ActiveWindow, app_name: &str) -> Option<u32> {
    for rule in &config.rules_list {
        if rule.enabled
            && (rule.app_name.is_empty() || app_name.contains(&rule.app_name))
            && (rule.window_title.is_empty() || window.title.contains(&rule.window_title))
            && match &rule.process_name {
                Some(process_name) => {
                    process_name.is_empty() || window.app_name.contains(process_name)
                }
                None => true,
            }
        {
            return Some(rule.switch_to);
        }
    }

    None
}
