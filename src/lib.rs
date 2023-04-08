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
    sys: &mut Option<System>,
    config: &Value,
    prev_profile: Option<u8>,
    callback: &mut Option<Command>,
    os: &enums::OSIdent,
) -> Option<u8> {
    let window = match os {
        enums::OSIdent::UNSUPPORTED(script)
        | enums::OSIdent::LINUX(enums::LinuxServer::WAYLAND(script)) => {
            custom_active_window(script)
        }
        _ => get_active_window(),
    };

    if let Ok(window) = window {
        let app_name = get_app_name(sys, Pid::from(window.process_id as usize))
            .unwrap_or("unknown".to_string());

        if let Some(profile) = next_profile(&config, &window, &app_name) {
            if prev_profile.is_none() || profile != prev_profile.unwrap() {
                if let Ok(duckypad) = hid::init(&api) {
                    if let Ok(_) = goto_profile(&duckypad, profile) {
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
            .unwrap_or(&Value::Number(serde_json::Number::from_f64(0.0).unwrap()))
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
/// -p <PROFILE> [-a <APP_NAME>] [-t <TITLE>] [-n <PROCESS_NAME>]
/// ```
///
/// # Arguments
///
/// * `callback` - optional callback script to run on change
/// * `profile` - id of the profile on the duckypad (1 <= id <= 31)
/// * `window` - information about the active window
pub fn run_callback(
    callback: &mut Command,
    profile: u8,
    window: ActiveWindow,
    app_name: &String,
) -> () {
    let mut callback = callback.arg("-p").arg(profile.to_string());

    if app_name != "" {
        callback = callback.arg("-a").arg(app_name);
    }
    if window.title != "" {
        callback = callback.arg("-t").arg(window.title);
    }
    if window.process_name != "" {
        callback = callback.arg("-n").arg(window.process_name);
    }

    if let Err(err) = callback.spawn() {
        eprintln!("Failed to run callback: {}", err);
    }
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
pub fn next_profile(config: &Value, window: &ActiveWindow, app_name: &String) -> Option<u8> {
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
            let conf_process_name = item
                .get("process_name")
                .expect(ERR_STR)
                .as_str()
                .expect(ERR_STR);
            let conf_title = item.get("title").expect(ERR_STR).as_str().expect(ERR_STR);
            let conf_app_name = item
                .get("app_name")
                .expect(ERR_STR)
                .as_str()
                .expect(ERR_STR);

            if (conf_process_name.len() == 0 || window.process_name.contains(conf_process_name))
                && (conf_title.len() == 0 || window.title.contains(conf_title))
                && (conf_app_name.len() == 0 || app_name.contains(conf_app_name))
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
