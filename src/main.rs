#![warn(clippy::pedantic)]

use clap::Parser;
use duckypad_daemon::{config_file, enums, hid, read_config, switch_profile};
use std::{env, path::PathBuf, process::Command};
use sysinfo::{ProcessRefreshKind, RefreshKind, System, SystemExt};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to a config file to use
    #[arg(short, long, default_value = None)]
    config: Option<PathBuf>,

    /// Wait for <WAIT> seconds and retry if device isn't connected on daemon startup
    #[arg(short, long, default_value = None)]
    wait: Option<u64>,

    /// Path to an executable to call when switching profile
    /// CALLBACK -p <PROFILE> [-a <APP_NAME>] [-t <TITLE>] [-n <PROCESS_NAME>]
    #[arg(short = 'b', long, default_value = None, verbatim_doc_comment)]
    callback: Option<PathBuf>,

    /// Path to an executable to call periodically about active window information on platforms without native APIs
    /// Output must be a JSON with keys: title & process_name
    #[arg(short = 's', long, default_value = None, verbatim_doc_comment)]
    window_script: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    // create Command without args or spawning to use in `run_callback` (lib.rs)
    let mut callback = args.callback.map(Command::new);

    let config_path = config_file(args.config);
    let config = read_config(&config_path);

    let api = hidapi::HidApi::new().expect("Failed to connect to HidApi.");

    {
        let duckypad = if let Some(wait) = args.wait {
            loop {
                if let Ok(dev) = hid::init(&api) {
                    break dev;
                }

                eprintln!("Failed to connect to duckyPad. Retrying in {wait} seconds!");
                std::thread::sleep(std::time::Duration::from_secs(wait));
            }
        } else {
            hid::init(&api).expect(
                "Failed to connect to duckyPad. See --help if you want to enable auto-retrying.",
            )
        };

        let info = hid::info(&duckypad);
        println!(
            "Model: {}\tSerial: {}\tFirmware: {}",
            info.model, info.serial, info.firmware
        );
    }

    let os = match env::consts::OS {
        "macos" => enums::OSIdent::MACOS,
        "windows" => enums::OSIdent::WINDOWS,
        "linux" => {
            if let Some(script) = args.window_script {
                enums::OSIdent::LINUX(enums::LinuxServer::WAYLAND(script))
            } else if env::var("WAYLAND_DISPLAY").is_ok() {
                panic!("Wayland has no proper API for active window information. See --window-script,-s as well as the readme!")
            } else {
                enums::OSIdent::LINUX(enums::LinuxServer::XORG)
            }
        }
        _ => {
            if let Some(script) = args.window_script {
                enums::OSIdent::UNSUPPORTED(script)
            } else {
                panic!("Unsupported platform: See --window-script,-s as well as the readme!")
            }
        }
    };

    let mut sys = if System::IS_SUPPORTED {
        Some(System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::new()),
        ))
    } else {
        None
    };

    let mut prev_profile: Option<u8> = None;

    loop {
        prev_profile = switch_profile(&api, &mut sys, &config, prev_profile, &mut callback, &os);
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}
