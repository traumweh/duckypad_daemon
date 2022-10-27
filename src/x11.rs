use sysinfo::{Pid, ProcessExt, System, SystemExt};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt};
use x11rb::rust_connection::RustConnection;

pub fn active_window() -> (Option<String>, Option<String>) {
    let (con, screen) = x11rb::connect(None).expect("Couldn't connect to X11 server");
    let root = con.setup().roots[screen].root;

    let net_active_window = get_atom(&con, b"_NET_ACTIVE_WINDOW");
    let net_wm_name = get_atom(&con, b"_NET_WM_NAME");
    let net_wm_pid = get_atom(&con, b"_NET_WM_PID");
    let utf8_string = get_atom(&con, b"UTF8_STRING");
    let cardinal: Atom = AtomEnum::CARDINAL.into();

    let window: Atom = AtomEnum::WINDOW.into();
    let active_window = con
        .get_property(false, root, net_active_window, window, 0, 1)
        .expect("Couldn't get property from X11 server")
        .reply()
        .expect("Couldn't get reply for property from X11 server");

    let active_window = if active_window.length == 1 && active_window.format == 0x20 {
        let tmp = active_window.value32().expect("Invalid message.").next();

        if tmp.is_none() {
            return (None, None);
        }

        tmp.unwrap()
    } else {
        con.get_input_focus()
            .expect("Failed to get input focus")
            .reply()
            .expect("Failed to receive X11 input focus")
            .focus
    };

    let name = con
        .get_property(false, active_window, net_wm_name, utf8_string, 0, u32::MAX)
        .expect("Missing property _NET_WM_NAME")
        .reply()
        .expect("Failed to get reply for _NET_WM_NAME");
    let name = std::str::from_utf8(&name.value)
        .expect("Invalid UTF-8")
        .to_string();

    let pid = con
        .get_property(false, active_window, net_wm_pid, cardinal, 0, u32::MAX)
        .expect("Missing property _NET_WM_PID")
        .reply()
        .expect("Failed to get reply for _NET_WM_PID");
    let pid = i32::from_le_bytes(match pid.value[..].try_into() {
        Ok(x) => x,
        Err(_) => [0; 4],
    });

    let cmd = if pid == 0 {
        None
    } else {
        let mut sys = System::new_all();
        sys.refresh_processes();
        let process = sys.process(Pid::from(pid));

        if process.is_none() {
            return (None, None);
        }

        Some(process.unwrap().name().to_string())
    };
    (cmd, Some(name))
}

fn get_atom(con: &RustConnection, property: &[u8]) -> Atom {
    let res = con
        .intern_atom(false, property)
        .expect("Failed to get atom")
        .reply()
        .expect("Failed to get reply for atom");

    res.atom
}
