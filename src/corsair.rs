//! Corsair "Wireless V2" headsets over their dongle: VOID WIRELESS V2, VIRTUOSO MAX and
//! HS80 MAX.
//!
//! Protocol as documented by HeadsetControl (`corsair_void_v2w`). Packets are 64 bytes on
//! interface 4, written without a report id: `02 <endpoint> <op> <command> …`, where
//! endpoint 0x08 is the dongle and 0x09 the headset.
//!
//! 1. Wake: ask the dongle for its firmware (`02 08 02 13`) and heartbeat it (`02 08 02 12`),
//!    then heartbeat the headset (`02 09 02 12`). No answer means the headset is off. This
//!    is the light handshake that doesn't switch the headset to software mode (which pops).
//! 2. Battery: `02 09 02 0F`; bytes 4-5 of the reply are 1-1000, little endian. The dongle
//!    sometimes answers with something else (the paired headset's id), so out-of-range
//!    replies are retried.
//!
//! No charging flag is known for these.

use crate::device::{Kind, Observation, State};
use crate::diag;
use hidapi::{HidApi, HidDevice};
use std::collections::HashMap;
use std::ffi::CString;
use std::time::Duration;

const VID: u16 = 0x1B1C;
const INTERFACE: i32 = 4;
const DONGLE: u8 = 0x08;
const HEADSET: u8 = 0x09;
const WAIT: Duration = Duration::from_millis(500);
const ATTEMPTS: usize = 3;

/// (product id, name)
const DEVICES: &[(u16, &str)] = &[
    (0x2A08, "Corsair VOID WIRELESS V2"),
    (0x2A02, "Corsair VIRTUOSO MAX WIRELESS"),
    (0x0A97, "Corsair HS80 MAX WIRELESS"),
];

struct Dongle {
    dev: HidDevice,
    key: String,
    name: &'static str,
}

#[derive(Default)]
pub struct Corsair {
    open: HashMap<CString, Dongle>,
}

impl Corsair {
    pub fn scan(&mut self, api: &HidApi) -> Vec<Observation> {
        // One collection per dongle: the vendor-page one on interface 4 if there are several.
        let mut present: Vec<(CString, u16, &'static str, bool)> = Vec::new();
        for d in api.device_list().filter(|d| d.vendor_id() == VID && d.interface_number() == INTERFACE) {
            let Some((pid, name)) = DEVICES.iter().find(|(p, _)| *p == d.product_id()) else { continue };
            let vendor = d.usage_page() >= 0xFF00;
            match present.iter_mut().find(|(_, p, ..)| p == pid) {
                Some(slot) if vendor && !slot.3 => *slot = (d.path().to_owned(), *pid, name, vendor),
                Some(_) => {}
                None => present.push((d.path().to_owned(), *pid, name, vendor)),
            }
        }
        self.open.retain(|path, _| present.iter().any(|(p, ..)| p == path));

        let mut out = Vec::new();
        for (path, pid, name, _) in present {
            if !self.open.contains_key(&path) {
                let key = format!("corsair:{pid:04x}");
                match api.open_path(&path) {
                    Ok(dev) => {
                        self.open.insert(path.clone(), Dongle { dev, key, name });
                    }
                    Err(e) => {
                        diag::event(&key, format!("open failed: {e}"));
                        continue;
                    }
                }
            }
            let d = &self.open[&path];
            let state = match query(&d.dev) {
                Ok(Reading::Off) => {
                    diag::event(&d.key, "headset didn't answer the heartbeat (off)");
                    State::Off
                }
                Ok(Reading::Percent(percent)) => {
                    diag::event(&d.key, "battery via 02 09 02 0F");
                    State::Battery { percent, charging: false }
                }
                Ok(Reading::Invalid(raw)) => {
                    diag::event(&d.key, format!("no plausible battery value (last: {raw})"));
                    State::Unreachable
                }
                Err(e) => {
                    // Unplugged or the handle went bad: reopen on the next scan.
                    diag::event(&d.key, format!("i/o failed: {e}"));
                    self.open.remove(&path);
                    continue;
                }
            };
            out.push(Observation { id: d.key.clone(), name: Some(d.name.into()), kind: Some(Kind::Headset), state });
        }
        out
    }
}

enum Reading {
    Off,
    Percent(u8),
    /// The last raw 1-1000 value, which was out of range.
    Invalid(u16),
}

fn query(dev: &HidDevice) -> hidapi::HidResult<Reading> {
    let mut buf = [0u8; 64];
    flush(dev, &mut buf)?;
    send(dev, DONGLE, 0x13)?; // dongle firmware
    send(dev, DONGLE, 0x12)?; // dongle heartbeat
    flush(dev, &mut buf)?;
    send(dev, HEADSET, 0x12)?; // headset heartbeat
    if dev.read_timeout(&mut buf, WAIT.as_millis() as i32)? == 0 {
        return Ok(Reading::Off);
    }
    flush(dev, &mut buf)?;

    let mut raw = 0;
    for _ in 0..ATTEMPTS {
        send(dev, HEADSET, 0x0F)?;
        let n = dev.read_timeout(&mut buf, WAIT.as_millis() as i32)?;
        raw = if n >= 6 { u16::from_le_bytes([buf[4], buf[5]]) } else { 0 };
        if let Some(p) = percent(raw) {
            return Ok(Reading::Percent(p));
        }
    }
    Ok(Reading::Invalid(raw))
}

/// Writes `02 <endpoint> 02 <command>` (a read request) as a 64-byte report without an id.
fn send(dev: &HidDevice, endpoint: u8, command: u8) -> hidapi::HidResult<()> {
    let mut r = [0u8; 65];
    r[1..5].copy_from_slice(&[0x02, endpoint, 0x02, command]);
    dev.write(&r).map(|_| ())
}

fn flush(dev: &HidDevice, buf: &mut [u8]) -> hidapi::HidResult<()> {
    while dev.read_timeout(buf, 5)? > 0 {}
    Ok(())
}

fn percent(raw: u16) -> Option<u8> {
    (1..=1000).contains(&raw).then_some((raw / 10) as u8)
}

#[cfg(test)]
mod tests {
    #[test]
    fn percent() {
        assert_eq!(super::percent(1000), Some(100));
        assert_eq!(super::percent(735), Some(73));
        assert_eq!(super::percent(1), Some(0));
        assert_eq!(super::percent(0), None);
        assert_eq!(super::percent(0x0A97), None); // a headset id instead of a level
    }
}
