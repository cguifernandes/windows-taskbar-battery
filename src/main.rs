#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bluetooth;
mod device;
mod logitech;
mod notify;
mod render;
mod settings;
mod widget;

pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Initializes COM (multithreaded) on the calling thread, needed for the Bluetooth APIs.
pub(crate) fn init_com() {
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
    }
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        // Debug helpers (they print only in debug builds; release builds have no console).
        Some("--test-toast") => {
            init_com();
            settings::register_app_id();
            if let Err(e) = notify::show("Low battery", "G Pro Wireless is at 12%. Time to charge it.") {
                eprintln!("toast failed: {e}");
            }
        }
        Some("--list") => logitech::hidpp::list(&hidapi::HidApi::new().expect("hidapi")),
        Some("--once") => {
            init_com();
            let api = hidapi::HidApi::new().expect("hidapi");
            let mut obs = logitech::Logitech::new().scan(&api);
            obs.extend(bluetooth::scan());
            for r in device::Tracker::default().update(obs) {
                println!("{:<10} {:<36} {}  [{}]", format!("{:?}", r.kind), r.name, r.status, r.id);
            }
        }
        Some("--demo") if cfg!(debug_assertions) => {
            widget::DEMO.store(true, std::sync::atomic::Ordering::Relaxed);
            widget::run();
        }
        _ => widget::run(),
    }
}
