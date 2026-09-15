# Sable — engineering TODO

Branch: `fix/p0-real-telemetry`
Rule: a task is done only when the number on screen can be explained by a real Windows API path. No stub that returns `Ok(default)`.

## P0 — telemetry that is not a lie

- [x] **T1 GPU adapter + VRAM + usage**
- [x] **T2 Preset rollback actually restores**
- [x] **T3 FPS belongs to the game PID**
  - `get_active_metrics` prefers foreground PID if that exe is in the installed-game catalog
  - else most recent Present among catalog matches
  - compositor/browser/launcher PIDs are never the target
  - dead PIDs evicted from the ring map
  - first Present is a timestamp only — no fake 16.67 ms frame
  - `FrameMetrics.target_process` is the exe name actually scored
- [ ] **T4 ETW Present events that exist in 2026**
  - DXGI Present event IDs used by PresentMon, not only opcode 1
  - treat `EnableTraceEx2` / `StartTrace` failures as errors

## P1 — product honesty

- [ ] **T5 Updater capability**
- [ ] **T6 Overlay HUD design**
- [ ] **T7 Shell design**
