use duckypad_daemon::{config_file, goto_profile, hid, next_profile, read_config};
use notify::{watcher, DebouncedEvent::Write, RecursiveMode, Watcher};
use std::sync::{mpsc::channel, Arc, Mutex};
use std::thread;

fn main() {
    let config = Arc::new(Mutex::new(read_config()));
    let config_thread = Arc::clone(&config);

    thread::spawn(move || {
        let (tx, rx) = channel();
        let mut watcher = watcher(tx, std::time::Duration::from_secs(10))
            .expect("Failed to start config file watcher");
        watcher
            .watch(config_file(), RecursiveMode::NonRecursive)
            .unwrap_or_else(|err| {
                panic!(
                    "Failed to watch file: '{}'\nGot error: {:?}",
                    config_file().display(),
                    err
                )
            });

        loop {
            match rx.recv() {
                Ok(event) => {
                    eprintln!("Received watcher event: {:?}", event);

                    if let Write(_) = event {
                        let mut config_lock = config_thread.lock().expect("Failed to lock mutex.");
                        *config_lock = read_config();
                    }
                }
                Err(err) => panic!(
                    "Failed to watch file: '{}'\nGot error: {:?}",
                    config_file().display(),
                    err
                ),
            }
        }
    });

    println!("duckypad daemon started!");

    let api = hidapi::HidApi::new().expect("Failed to connect to HidApi.");

    {
        let duckypad = hid::init(&api).expect("Failed to connect to duckyPad.");
        let info = hid::info(&duckypad).expect("Failed to connect to duckyPad.");
        println!(
            "Model: {}\tSerial: {}\tFirmware: {}",
            info.model, info.serial, info.firmware
        );
    }

    let mut prev_profile: Option<u8> = None;

    loop {
        let profile = next_profile(&Arc::clone(&config).lock().expect("Failed to lock mutex."));

        if profile.is_some()
            && (prev_profile.is_none() || profile.unwrap() != prev_profile.unwrap())
        {
            if let Ok(duckypad) = hid::init(&api) {
                if let Ok(_) = goto_profile(&duckypad, profile.unwrap()) {
                    prev_profile = profile;
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}
