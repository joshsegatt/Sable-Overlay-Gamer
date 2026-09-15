# Sable — engineering TODO

Branch: `fix/p0-real-telemetry`

- [x] T1 GPU adapter + VRAM + usage
- [x] T2 Preset rollback actually restores
- [x] T3 FPS belongs to the game PID
- [x] T4 ETW Present events that exist in 2026
- [x] T5 Updater capability
  - `updater:default` on the main window capability
  - window label `main` matches the capability
  - `createUpdaterArtifacts: true` so MSI/NSIS emit `.sig`
  - check-for-update no longer reports "up to date" when `check()` throws
- [ ] T6 Overlay HUD design
- [ ] T7 Shell design
