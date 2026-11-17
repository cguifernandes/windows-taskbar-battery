//! Optional integration with HeadsetControl (https://github.com/Sapd/HeadsetControl), which
//! supports many headsets (SteelSeries, HyperX, Corsair, Logitech, Audeze…).
//!
//! It runs as a separate program (`headsetcontrol -o json`), so it isn't bundled and its
//! GPL license doesn't apply to this project. It's used only if found next to our exe or
//! on PATH.

use crate::device::{Kind, Observation, State};
use serde::Deserialize;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const EXE_NAMES: &[&str] = &["headsetcontrol.exe", "headsetcontrol-windows-x86_64.exe"];
const TIMEOUT: Duration = Duration::from_secs(5);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Deserialize)]
struct Output {
    #[serde(default)]
    devices: Vec<Device>,
}

#[derive(Deserialize)]
struct Device {
    #[serde(default)]
    device: String,
    #[serde(default)]
    product: String,
    #[serde(default)]
    id_vendor: String,
    #[serde(default)]
    id_product: String,
    battery: Option<Battery>,
}

#[derive(Deserialize)]
struct Battery {
    status: String,
    level: i32,
}

/// Locates the HeadsetControl executable, next to our exe or on PATH.
pub fn find() -> Option<PathBuf> {
    let beside = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
    let path_dirs = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default();
    beside
        .into_iter()
        .chain(path_dirs)
        .flat_map(|dir| EXE_NAMES.iter().map(move |n| dir.join(n)))
        .find(|p| p.is_file())
}

/// Runs HeadsetControl and returns its headsets. `skip_pid` filters out devices another
/// source already reports (e.g. Logitech headsets read over HID++).
pub fn scan(exe: &Path, skip_pid: impl Fn(u16) -> bool) -> Vec<Observation> {
    let Some(json) = run(exe) else { return Vec::new() };
    let output: Output = match serde_json::from_str(&json) {
        Ok(o) => o,
        Err(e) => {
            if cfg!(debug_assertions) {
                eprintln!("[headsetcontrol] bad json: {e}");
            }
            return Vec::new();
        }
    };
    output
        .devices
        .into_iter()
        .filter_map(|d| {
            let pid = parse_hex(&d.id_product)?;
            if skip_pid(pid) {
                return None;
            }
            let battery = d.battery?;
            let state = match battery.status.as_str() {
                "BATTERY_AVAILABLE" => State::Battery { percent: battery.level.clamp(0, 100) as u8, charging: false },
                "BATTERY_CHARGING" => State::Battery { percent: battery.level.clamp(0, 100) as u8, charging: true },
                _ => State::Off, // unavailable: headset off
            };
            if matches!(state, State::Battery { .. }) && battery.level < 0 {
                return None;
            }
            let name = if d.device.is_empty() { d.product } else { d.device };
            Some(Observation {
                id: format!("hc:{}:{pid:04x}", d.id_vendor.trim_start_matches("0x")),
                kind: Some(match Kind::from_name(&name) {
                    Kind::Earbuds => Kind::Earbuds,
                    _ => Kind::Headset,
                }),
                name: Some(name),
                state,
            })
        })
        .collect()
}

fn run(exe: &Path) -> Option<String> {
    let mut child = Command::new(exe)
        .args(["-o", "json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .ok()?;
    let deadline = Instant::now() + TIMEOUT;
    while child.try_wait().ok()?.is_none() {
        if Instant::now() > deadline {
            let _ = child.kill();
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output().ok()?;
    String::from_utf8(out.stdout).ok()
}

fn parse_hex(s: &str) -> Option<u16> {
    u16::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_headsetcontrol_json() {
        let json = r#"{"name":"HeadsetControl","version":"4.3.0","device_count":2,"devices":[
            {"status":"success","device":"SteelSeries Arctis Nova 7","vendor":"SteelSeries","product":"Arctis Nova 7",
             "id_vendor":"0x1038","id_product":"0x2202","battery":{"status":"BATTERY_CHARGING","level":64}},
            {"status":"success","device":"Logitech PRO X","vendor":"Logitech","product":"PRO X",
             "id_vendor":"0x046d","id_product":"0x0aba","battery":{"status":"BATTERY_AVAILABLE","level":90}}]}"#;
        let output: Output = serde_json::from_str(json).unwrap();
        assert_eq!(output.devices.len(), 2);
        assert_eq!(parse_hex(&output.devices[0].id_product), Some(0x2202));
        assert_eq!(output.devices[0].battery.as_ref().unwrap().level, 64);
    }
}
