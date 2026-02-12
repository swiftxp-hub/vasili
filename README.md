# VASILI

> "Give me a ping, Vasili. One ping only."

Vasili is a lightweight, asynchronous network monitor written in Rust. It runs in the terminal (TUI) and is designed to help you pinpoint the source of network latency (lag).

Unlike standard ping tools, Vasili simultaneously tracks your **Target** (e.g., Google DNS) and your **Gateway** (Next Hop). By comparing these two metrics in real-time, you can e.g. immediately identify if a lag spike is caused by your ISP/Modem (Target only spikes) or your local hardware/configuration (Gateway and Target spike together).

## Features

* **Dual Monitoring:** Pings a target IP and the gateway (next hop) in parallel.
  **Smart Interval:** The gateway is probed at 2x the frequency of the target to detect local micro-stutters with higher resolution.
* **Real-time TUI:** Visualizes latency, jitter, and packet loss using high-performance terminal charts (powered by `ratatui`).
* **Daemon Mode:** Run Vasili in the background (headless) without the TUI. Perfect for long-term monitoring on servers or Raspberry Pis.
* **Jitter Analysis:** Calculates current jitter and records P25, P75, and P95 percentile latency stats.
* **Grading System:** Automatically grades your connection stability (S, A, B, C, F) based on packet loss and latency spikes.
                      *Note:* These grades are only intended to provide an initial rough guide and cannot replace a thorough examination of the data.
* **History & Zoom:** Scroll through past data and zoom the time axis in and out dynamically.
* **CSV Logging:** Automatically saves all ping data to a CSV file for later analysis (e.g. in Google Sheets).
* **Lightweight:** Built with Rust and `tokio` for minimal resource usage, making it suitable for embedded devices (e.g. running directly on routers).

---

![Vasili 1.0.2 screenshot](https://raw.githubusercontent.com/swiftxp-hub/vasili/refs/heads/main/assets/vassili-102-screenshot.png)

---

## Installation

### Prerequisites
* Rust toolchain (cargo)

### Build from Source

```bash
git clone https://github.com/swiftxp-hub/vasili.git
cd vasili
cargo build --release
```

The binary will be located at `target/release/vasili`.

### Cross-Compilation (for Routers)
Vasili is designed to run on Linux-based routers (e.g. OpenWrt, Asuswrt-Merlin). To build for these targets (e.g., ARMv7 or AArch64), use `cross`:

```bash
# Example for ARMv7 (many Asus Routers)
cross build --target armv7-unknown-linux-musleabihf --release
```

## Usage

Run the tool from your terminal.

```bash
./vasili [OPTIONS]
```

**Note regarding permissions:**
Vasili uses ICMP packets. On most modern Linux systems (and routers logged in as root), this works out of the box. If you encounter permission errors ("Operation not permitted"), you may need to run it with `sudo` or configure your system to allow unprivileged pings.

```bash
# Only if you get permission errors:
sudo ./vasili
```

### Options

* `-t, --target <IP>`: Specify a target IP (defaults to a random choice from a reliable pool like 1.1.1.1 or 8.8.8.8).
* `-m, --mode <MODE>`: Presets for interval speed.
    * `Gaming` (50ms interval, default)
    * `Standard` (500ms interval)
    * `Monitor` (5000ms interval)
* `-i, --interval <DURATION>`: Manually set the ping interval (e.g., `500ms`, `1s`, `30s`, `1m`). The Gateway will automatically be pinged at half this interval (double frequency). Overrides `mode`.
* `-d, --duration <DURATION>`: Stop automatically after a set time (e.g., `30s`, `10m`, `1h`).
* `-D, --daemon`: Run in headless mode (no TUI). Logs data directly to CSV. (*Note:* Cannot be used with `--no-csv`).
* `--no-gateway`: Disable gateway monitoring (target only).
* `--no-csv`: Disable saving ping data to a CSV file.

### Examples

```bash
# Default gaming mode (50ms interval)
./vasili

# Monitor a specific server for 1 hour
./vasili --target 1.1.1.1 --duration 1h

# Low frequency monitoring
./vasili --mode Monitor

# Run in background (headless)
./vasili --daemon --target 1.1.1.1
```

## Controls

The interface is interactive. Use the following keys during operation:

* **Q**: Quit the application.
* **Space**: Pause / Resume the graph (pings continue in background).
* **+ / -** or **Up / Down**: Zoom the time axis (seconds displayed).
* **Left / Right**: Scroll through the history (time travel).

## Understanding the Output

### The Graph
* **Green Line:** Target Latency.
* **Yellow Line:** Target Jitter.
* **Red Block:** Target Packet Loss (Timeout).
* **Blue Line:** Gateway Latency.
* **Light Cyan Line:** Gateway Jitter.
* **Magenta Block:** Gateway Packet Loss (Timeout).

### The Logic
1.  **If Target spikes but Gateway is flat:** The issue is likely external (ISP, Modem, or the route to the server).
2.  **If both Target and Gateway spike simultaneously:** The issue is local. Your router CPU might be overloaded, or the local link (Wi-Fi/LAN) is congested.

## About this Project

**Note:** This is my first project written in Rust. It was created as a learning exercise to understand the language. The code and architecture were developed with the assistance of AI.

## License

This project is licensed under the MIT License.