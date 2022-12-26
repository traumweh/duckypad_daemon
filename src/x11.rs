use sysinfo::{Pid, ProcessExt, ProcessRefreshKind, System, SystemExt};
use x11rb::connection::Connection;
use x11rb::properties::WmClass;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt};
use x11rb::rust_connection::RustConnection;

pub fn active_window(
    con: &RustConnection,
    screen: usize,
    sys: &mut System,
) -> (Option<String>, Option<String>, Option<String>) {
    let root = con.setup().roots[screen].root;

    let net_active_window = get_atom(&con, b"_NET_ACTIVE_WINDOW");

    let window: Atom = AtomEnum::WINDOW.into();
    let active_window = con
        .get_property(false, root, net_active_window, window, 0, 1)
        .expect("Couldn't get property from X11 server")
        .reply()
        .expect("Couldn't get reply for property from X11 server");

    let active_window = if active_window.length == 1 && active_window.format == 0x20 {
        let tmp = active_window.value32().expect("Invalid message.").next();

        if tmp.is_none() {
            return (None, None, None);
        }

        tmp.unwrap()
    } else {
        con.get_input_focus()
            .expect("Failed to get input focus")
            .reply()
            .expect("Failed to receive X11 input focus")
            .focus
    };

    let cmd = match get_wm_pid(&con, active_window) {
        Some(pid) => get_cmd(sys, pid),
        None => None,
    };

    (
        cmd,
        get_wm_class(&con, active_window),
        get_wm_name(&con, active_window),
    )
}

fn get_wm_class(con: &RustConnection, active_window: u32) -> Option<String> {
    let wm_class = WmClass::get(con, active_window);

    if let Ok(wm_class) = wm_class {
        if let Ok(Some(wm_class)) = wm_class.reply_unchecked() {
            if let Ok(class) = std::str::from_utf8(wm_class.class()) {
                return Some(class.to_string());
            }
        }
    }

    None
}

fn get_wm_name(con: &RustConnection, active_window: u32) -> Option<String> {
    let net_wm_name = get_atom(&con, b"_NET_WM_NAME");
    let utf8_string = get_atom(&con, b"UTF8_STRING");

    if let Ok(property) =
        con.get_property(false, active_window, net_wm_name, utf8_string, 0, u32::MAX)
    {
        if let Ok(reply) = property.reply() {
            if let Ok(str) = std::str::from_utf8(&reply.value) {
                return Some(str.to_string());
            }
        }
    }

    None
}

fn get_wm_pid(con: &RustConnection, active_window: u32) -> Option<i32> {
    let net_wm_pid = get_atom(&con, b"_NET_WM_PID");
    let cardinal: Atom = AtomEnum::CARDINAL.into();

    if let Ok(property) = con.get_property(false, active_window, net_wm_pid, cardinal, 0, u32::MAX)
    {
        if let Ok(reply) = property.reply() {
            return Some(i32::from_le_bytes(match reply.value[..].try_into() {
                Ok(arr) => arr,
                Err(_) => [0; 4],
            }));
        }
    }

    None
}

fn get_cmd(sys: &mut System, pid: i32) -> Option<String> {
    if pid != 0 {
        let pid = Pid::from(pid);
        sys.refresh_process_specifics(pid, ProcessRefreshKind::new());
        let process = sys.process(pid);

        if process.is_some() {
            return Some(process.unwrap().name().to_string());
        }
    }

    None
}

fn get_atom(con: &RustConnection, property: &[u8]) -> Atom {
    let res = con
        .intern_atom(false, property)
        .expect("Failed to get atom")
        .reply()
        .expect("Failed to get reply for atom");

    res.atom
}
