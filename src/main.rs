pub mod x11;

use clap::Parser;
use duckypad_daemon::{config_file, goto_profile, hid, next_profile, read_config};
use notify::{watcher, DebouncedEvent::Write, RecursiveMode, Watcher};
use std::sync::mpsc::{channel, TryRecvError};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Config file to use
    #[arg(short, long, default_value = None)]
    config: Option<String>,

    /// Wait for x seconds and retry if device isn't connected on daemon startup
    #[arg(short, long, default_value = None)]
    wait: Option<u64>,
}

fn main() {
    let args = Args::parse();
    let config_path = config_file(&args.config);
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

    let mut prev_profile: Option<u8> = None;
    let ((con, screen), mut sys) = x11::init();

    const RECV_INTERVAL: std::time::Duration = std::time::Duration::from_secs(10);
    const WAIT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);
    const COUNTER_RESET: std::time::Duration = std::time::Duration::from_secs(0);
    let mut recv_counter = COUNTER_RESET;

    loop {
        let profile = next_profile(&config, &con, screen, &mut sys);

        if profile.is_some()
            && (prev_profile.is_none() || profile.unwrap() != prev_profile.unwrap())
        {
            if let Ok(duckypad) = hid::init(&api) {
                if let Ok(_) = goto_profile(&duckypad, profile.unwrap()) {
                    prev_profile = profile;
                }
            }
        }

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
