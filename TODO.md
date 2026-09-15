# Sable — engineering TODO

Branch: `fix/p0-real-telemetry`

- [x] T1 GPU adapter + VRAM + usage
- [x] T2 Preset rollback actually restores
- [x] T3 FPS belongs to the game PID
- [x] T4 ETW Present events that exist in 2026
  - DXGI Present_Start id 0x2A (42) and PresentMultiplaneOverlay_Start id 0x37 (55)
  - D3D9 Present_Start id 0x01 on provider {783ACA0A-790E-4D7F-8451-AA850511C6B9}
  - opcode 1 alone is rejected (SwapChain_Start / ResizeBuffers_Start are not frames)
  - DXGI_PRESENT_TEST (flag 0x1) is not counted
  - StartTrace / EnableTraceEx2 / OpenTrace failures return Err, not Ok(())
  - Vulkan/GL still need DxgKrnl PresentHistory — not claimed here
- [ ] T5 Updater capability
- [ ] T6 Overlay HUD design
- [ ] T7 Shell design
