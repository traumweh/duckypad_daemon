use clap::Parser;
use duckypad_daemon::{config_file, enums, hid, read_config, switch_profile};
use notify::{watcher, DebouncedEvent::Write, RecursiveMode, Watcher};
use std::{
    env,
    path::PathBuf,
    process::Command,
    sync::mpsc::{channel, TryRecvError},
};

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
    /// CALLBACK -p <PROFILE> [-c <CMD>] [-w <WM_CLASS>] [-n <WM_NAME>]
    #[arg(short = 'b', long, default_value = None, verbatim_doc_comment)]
    callback: Option<PathBuf>,

    /// Path to an executable to call periodically about wayland active window information
    /// Output must be a JSON with keys: WM_CLASS, MW_NAME & PID
    #[arg(short = 'y', long, default_value = None)]
    wayland_script: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    // create Command without args or spawning to use in `run_callback` (lib.rs)
    let mut callback = if let Some(callback) = args.callback {
        Some(Command::new(callback))
    } else {
        None
    };

    let config_path = config_file(args.config);
    let mut config = read_config(&config_path);

    let (tx, rx) = channel();
    let mut watcher = watcher(tx, std::time::Duration::from_secs(10))
        .expect("Failed to start config file watcher");
    watcher
        .watch(&config_path, RecursiveMode::NonRecursive)
        .unwrap_or_else(|err| {
            panic!(
                "Failed to watch file: '{}'\nGot error: {:?}",
                config_path.display(),
                err
            )
        });

    println!("duckypad daemon started!");

    let api = hidapi::HidApi::new().expect("Failed to connect to HidApi.");

    {
        let duckypad = if let Some(wait) = args.wait {
            loop {
                if let Ok(dev) = hid::init(&api) {
                    break dev;
                }

                eprintln!(
                    "Failed to connect to duckyPad. Retrying in {} seconds!",
                    wait
                );
                std::thread::sleep(std::time::Duration::from_secs(wait));
            }
        } else {
            hid::init(&api).expect(
                "Failed to connect to duckyPad. See --help if you want to enable auto-retrying.",
            )
        };

        let info = hid::info(&duckypad).expect("Failed to connect to duckyPad to retrieve device information. Maybe you are missing device permissions?");
        println!(
            "Model: {}\tSerial: {}\tFirmware: {}",
            info.model, info.serial, info.firmware
        );
    }

    let os = match env::consts::OS {
        "macos" => enums::OSIdent::MACOS,
        "windows" => enums::OSIdent::WINDOWS,
        "linux" => {
            // if env::var("WAYLAND_DISPLAY").is_ok() {
            if let Some(script) = args.wayland_script {
                enums::OSIdent::LINUX(enums::LinuxServer::WAYLAND(script))
            } else {
                enums::OSIdent::LINUX(enums::LinuxServer::XORG)
            }
        }
        _ => enums::OSIdent::UNSUPPORTED,
    };

    match &os {
        enums::OSIdent::UNSUPPORTED => panic!("You are running an unsupported OS!\n"),
        _ => (),
    };

    let mut prev_profile: Option<u8> = None;

    const RECV_INTERVAL: std::time::Duration = std::time::Duration::from_secs(10);
    const WAIT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);
    const COUNTER_RESET: std::time::Duration = std::time::Duration::from_secs(0);
    let mut recv_counter = COUNTER_RESET;

    loop {
        prev_profile = switch_profile(&api, &config, prev_profile, &mut callback, &os);

        recv_counter += WAIT_INTERVAL;
        std::thread::sleep(WAIT_INTERVAL);

        if recv_counter >= RECV_INTERVAL {
            recv_counter = COUNTER_RESET;
            match rx.try_recv() {
                Ok(event) => {
                    eprintln!("Received watcher event: {:?}", event);

                    if let Write(_) = event {
                        config = read_config(&config_path);
                    }
                }
                Err(TryRecvError::Empty) => (),
                Err(TryRecvError::Disconnected) => {
                    panic!("Failed to watch file: '{}'", config_path.display(),)
                }
            };
        }
    }
}
