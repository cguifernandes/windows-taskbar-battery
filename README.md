# battery-bar

Battery levels of your wireless mouse, headset, and keyboard, right on the Windows 11 taskbar.

![battery-bar on the taskbar](docs/screenshot.png)

A single ~300 KB executable written in Rust. It uses under 20 MB of RAM, has no runtime dependencies, and is idle between polls.

## Supported devices

| Devices | How | Notes |
|---|---|---|
| **Logitech**: Lightspeed, Unifying, and Bolt receivers, plus wireless headset dongles | HID++ protocol, detected automatically | When G HUB is installed, its per-device battery curves are used so percentages match G HUB. G HUB doesn't need to be running. |
| **Bluetooth LE** mice, keyboards, and gamepads | Standard GATT Battery Service | Polled every 60 s |
| **Classic Bluetooth** headsets/earbuds | The battery level Windows itself reports (as in Settings › Bluetooth) | |

Other brands' 2.4 GHz dongles (Razer, SteelSeries, Corsair, HyperX…) use their own proprietary protocols and aren't supported yet. Contributions welcome.

## Install

1. Download `battery-bar.exe` from the [latest release](../../releases/latest).
2. Put it somewhere permanent, e.g. `%LOCALAPPDATA%\Programs\battery-bar\`.
3. Run it. It registers itself to start with Windows.

The exe isn't code-signed, so SmartScreen may warn on first run: click **More info → Run anyway**.

## Usage

- **Right-click** the widget to show/hide individual devices, refresh, toggle *Start with Windows*, or exit.
- A dimmed `—` means the device isn't connected. Red means ≤ 15% and green means charging.
- A Logitech device that's asleep keeps showing its last reading. A sleeping device can't be told apart from one that's switched off, and the level doesn't change in either state.

**Uninstall:** right-click → uncheck *Start with Windows* → *Exit*, then delete the exe. Settings live in `HKCU\Software\battery-bar`.

## Build from source

Requires Rust (stable) and the MSVC build tools.

```sh
cargo build --release          # target/release/battery-bar.exe
cargo run -- --once            # debug: print detected devices and exit
cargo run -- --list            # debug: list Logitech HID interfaces
```

## How it works

- **Widget:** a layered (per-pixel alpha) child window of the taskbar, placed just left of the notification area. Windows 11 has no public API for taskbar widgets, so a future Windows update could break the placement.
- **Logitech:** talks HID++ 2.0 directly. It looks up the battery feature (`0x1004`, `0x1000`, `0x1001` or `0x1F20`) and reads the device name and type (`0x0005`). For voltage-only devices it converts millivolts to % using G HUB's discharge curves from `C:\ProgramData\LGHUB\depots`, or a generic Li-ion curve when G HUB isn't installed.
- **Bluetooth:** WinRT GATT APIs for BLE, plus the Windows battery device property for classic Bluetooth.

## Credits

Protocol details from [Solaar](https://github.com/pwr-Solaar/Solaar) and [HeadsetControl](https://github.com/Sapd/HeadsetControl).

Not affiliated with or endorsed by Logitech.

## License

[MIT](LICENSE)
