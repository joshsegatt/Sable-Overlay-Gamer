# Sable

Windows gaming overlay and performance toolkit.

[Download beta](https://github.com/joshsegatt/Sable-Overlay-Gamer/releases/latest)

Sable sits in the background, reads GPU / CPU telemetry (including ETW), and draws a light in-game overlay: FPS, frametime, load, VRAM. Per-game profiles, a one-click Windows tune, and a bottleneck read live in the same shell.

Built with Tauri 2, Rust, and React. Current public build is **v0.1.0-beta**.

## Features

- Overlay — FPS, frametime, GPU, CPU, VRAM. Position and opacity are configurable
- Game profiles — detect the title and apply the last settings you chose
- Benchmarks — record a session, compare runs, export CSV
- Hardware panel — thermals, memory pressure, obvious bottlenecks
- Optimizer — reversible Windows / GPU presets for latency and power plan

## Stack

- Rust workspace + Tauri 2
- React 19, Vite, Zustand, Recharts
- Windows 10 64-bit (1903+) / Windows 11, DirectX 11 GPU, ~150 MB disk

## Install

1. Open [Releases](https://github.com/joshsegatt/Sable-Overlay-Gamer/releases/latest).
2. Download `Sable_x64-setup.exe` or the MSI.
3. Run the installer and finish the short first-launch setup.

```bash
git clone https://github.com/joshsegatt/Sable-Overlay-Gamer.git
cd Sable-Overlay-Gamer
npm install
npm run tauri
```

Release build from source:

```bash
npm run build:release
```

## Privacy

Optional anonymous metrics exist only to improve presets, and only if you opt in at setup. Toggle off under Settings → Privacy. Nothing is sold.

## Status

Beta. Expect sharp edges. Issues and reproducible GPU / title reports are useful.
