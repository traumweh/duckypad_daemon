#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate)]

extern crate hidapi;
use hidapi::{HidApi, HidDevice, HidError};

pub struct DuckyPadInfo {
    pub model: String,
    pub serial: String,
    pub firmware: String,
}

pub const PC_TO_DUCKYPAD_HID_BUF_SIZE: usize = 64;
pub const DUCKYPAD_TO_PC_HID_BUF_SIZE: usize = 32;

const VENDOR_ID: u16 = 0x0483;
const PRODUCT_ID: u16 = 0xd11c;
const USAGE_PAGE: u16 = 0x0001;
const USAGE: u16 = 0x003a;

/// Initializes a connection to the duckypad and returns an `HidDevice`.
///
/// # Arguments
///
/// * `api` - connection to the hid api
///
/// # Errors
///
/// Will return `HidError` if the duckypad `HidDevice` cannot be opened or
/// set to non-blocking mode.
pub fn init(api: &HidApi) -> Result<HidDevice, HidError> {
    for item in api.device_list() {
        if item.vendor_id() == VENDOR_ID
            && item.product_id() == PRODUCT_ID
            && item.usage_page() == USAGE_PAGE
            && item.usage() == USAGE
        {
            let device = api.open_path(item.path())?;
            device.set_blocking_mode(false)?;
            return Ok(device);
        }
    }

    Err(HidError::HidApiError {
        message: format!(
            "Couldn't find device: (\
            vendor_id: {VENDOR_ID:#06x}, \
            product_id: {PRODUCT_ID:#06x}, \
            usage_page: {USAGE_PAGE:#06x}, \
            usage: {USAGE:#06x}"
        ),
    })
}

/// Returns device and firmware information about the connected duckypad.
/// Unavailable information will be replaced with "unknown".
///
/// # Arguments
///
/// * `device` - connected duckypad hid device
pub fn info(device: &HidDevice) -> DuckyPadInfo {
    let model = device
        .get_product_string()
        .unwrap_or_else(|_| Some("unknown".to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    let serial = device
        .get_serial_number_string()
        .unwrap_or_else(|_| Some("unknown".to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    let mut buf = [0x00; PC_TO_DUCKYPAD_HID_BUF_SIZE];
    buf[0] = 0x05;
    let firmware = match write(device, buf) {
        Ok(opt) => match opt {
            Some(buffer) => format!("{}.{}.{}", buffer[3], buffer[4], buffer[5]),
            None => "unknown".to_string(),
        },
        Err(_) => "unknown".to_string(),
    };

    DuckyPadInfo {
        model,
        serial,
        firmware,
    }
}

/// Returns a Result that either contains `DUCKYPAD_TO_PC_HID_BUF_SIZE` bytes
/// (u8) read from the conencted duckypad or a `HidError` indicating something
/// went wrong.
///
/// # Arguments
///
/// * `device` - connected duckypad hid device
///
/// # Errors
///
/// Will return `HidError` if reading from the duckypad `HidDevice` fails.
pub fn read(device: &HidDevice) -> Result<Option<[u8; DUCKYPAD_TO_PC_HID_BUF_SIZE]>, HidError> {
    let timer = std::time::Instant::now();

    while timer.elapsed() <= std::time::Duration::from_secs(5) {
        let mut buf = [0x00; DUCKYPAD_TO_PC_HID_BUF_SIZE];
        let res = device.read(&mut buf[..])?;

        if res > 0 {
            return Ok(Some(buf));
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    Ok(None)
}

/// Writes to the duckypad and returns a reply (see `read`).
///
/// # Arguments
///
/// * `device` - connected duckypad hid device
/// * `buf` - `PC_TO_DUCKYPAD_HID_BUF_SIZE` bytes (u8) to write to `device`
///
/// # Errors
///
/// Will return `HidError` if writing to or the follow-up reading from the
/// duckypad `HidDevice` fails.
pub fn write(
    device: &HidDevice,
    buf: [u8; PC_TO_DUCKYPAD_HID_BUF_SIZE],
) -> Result<Option<[u8; DUCKYPAD_TO_PC_HID_BUF_SIZE]>, HidError> {
    device.write(&buf)?;
    read(device)
}
