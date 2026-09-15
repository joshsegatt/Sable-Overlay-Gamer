# Sable — engineering TODO

Branch: `fix/p0-real-telemetry`
Rule: a task is done only when the number on screen can be explained by a real Windows API path. No stub that returns `Ok(default)`.

## P0 — telemetry that is not a lie

- [x] **T1 GPU adapter + VRAM + usage**
  - DXGI `IDXGIFactory6::EnumAdapterByGpuPreference(HIGH_PERFORMANCE)`
  - skip software / Basic Render adapters
  - VRAM total = `DedicatedVideoMemory`, used = `QueryVideoMemoryInfo.CurrentUsage`
  - NVIDIA: `NvAPI_EnumPhysicalGPUs` + `NvAPI_GPU_GetUsages` (kept loaded)
  - all vendors: PDH `\\GPU Engine(*)\\Utilization Percentage` fallback
- [ ] **T2 Preset rollback actually restores**
  - Game DVR / Game Bar / Game Mode restore the snapshotted bool, not the inverted one
  - do not mark a preset applied if the change is still `Ok(())` no-op
  - WSearch + Win32PrioritySeparation are Medium, not Low
- [ ] **T3 FPS belongs to the game PID**
  - ETW metrics keyed by detected game process, not latest Present on the machine
  - evict dead PIDs from the ring map
  - drop the fake 16.67 ms first frame
- [ ] **T4 ETW Present events that exist in 2026**
  - DXGI Present event IDs used by PresentMon, not only opcode 1
  - treat `EnableTraceEx2` / `StartTrace` failures as errors

## P1 — product honesty

- [ ] **T5 Updater capability** — add `updater:default` to Tauri capabilities so check-for-update is not a silent no-op
- [ ] **T6 Overlay HUD design** — in-game GDI overlay + settings preview: compact Afterburner-class HUD, not a 2017 admin panel
- [ ] **T7 Shell design** — dashboard / sidebar / cards: denser type, less chrome, no toy teal glow

## Done means

T1 is done when a laptop with iGPU+dGPU reports the dGPU name and a non-null GPU% while a 3D client is running.
