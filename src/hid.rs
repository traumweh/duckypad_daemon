extern crate hidapi;
use hidapi::{HidApi, HidDevice, HidError};

pub struct DuckyPadInfo {
    pub model: String,
    pub serial: String,
    pub firmware: String,
}

pub const PC_TO_DUCKYPAD_HID_BUF_SIZE: usize = 64;
pub const DUCKYPAD_TO_PC_HID_BUF_SIZE: usize = 32;

pub fn init(api: &HidApi) -> Result<HidDevice, HidError> {
    let device = api.open(0x0483, 0xd11c)?;
    device.set_blocking_mode(false)?;
    Ok(device)
}

pub fn info(device: &HidDevice) -> Result<DuckyPadInfo, HidError> {
    let mut buf = [0; PC_TO_DUCKYPAD_HID_BUF_SIZE];
    buf[0] = 5;

    let _ = write(device, buf)?;
    let mut firmware: String = buf[3].to_string();
    firmware.push('.');
    firmware.push_str(&buf[4].to_string());
    firmware.push('.');
    firmware.push_str(&buf[5].to_string());

    Ok(DuckyPadInfo {
        model: device
            .get_product_string()
            .unwrap_or_else(|_| Some("unknown".to_string()))
            .unwrap_or_else(|| "unknown".to_string()),
        serial: device
            .get_serial_number_string()
            .unwrap_or_else(|_| Some("unknown".to_string()))
            .unwrap_or_else(|| "unknown".to_string()),
        firmware,
    })
}

pub fn read(device: &HidDevice) -> Result<Option<[u8; DUCKYPAD_TO_PC_HID_BUF_SIZE]>, HidError> {
    let timer = std::time::Instant::now();

    while timer.elapsed() <= std::time::Duration::from_secs(5) {
        let mut buf = [0; DUCKYPAD_TO_PC_HID_BUF_SIZE];
        let res = device.read(&mut buf[..])?;

        if res > 0 {
            return Ok(Some(buf));
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    Ok(None)
}

pub fn write(
    device: &HidDevice,
    buf: [u8; PC_TO_DUCKYPAD_HID_BUF_SIZE],
) -> Result<Option<[u8; DUCKYPAD_TO_PC_HID_BUF_SIZE]>, HidError> {
    let _ = device.write(&buf)?;
    read(device)
}
