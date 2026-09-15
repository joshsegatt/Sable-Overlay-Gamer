// PresentMon event IDs (GameTechDev/PresentMon PresentData/ETW headers).
// DXGI: Microsoft_Windows_DXGI.h
// D3D9: Microsoft_Windows_D3D9.h

/// Microsoft-Windows-DXGI Present::Start
pub const DXGI_PRESENT_START: u16 = 0x002a;
/// Microsoft-Windows-DXGI PresentMultiplaneOverlay::Start
pub const DXGI_PRESENT_MPO_START: u16 = 0x0037;
/// Microsoft-Windows-D3D9 Present::Start
pub const D3D9_PRESENT_START: u16 = 0x0001;
/// DXGI_PRESENT_TEST — status probe, not a displayed frame
pub const DXGI_PRESENT_TEST: u32 = 0x1;

/// SwapChain::Start / ResizeBuffers::Start also use opcode 1.
/// They must not be counted as frames.
pub const DXGI_SWAPCHAIN_START: u16 = 0x000a;
pub const DXGI_RESIZEBUFFERS_START: u16 = 0x002d;

pub fn is_dxgi_present_start(id: u16) -> bool {
    id == DXGI_PRESENT_START || id == DXGI_PRESENT_MPO_START
}

pub fn is_d3d9_present_start(id: u16) -> bool {
    id == D3D9_PRESENT_START
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentmon_ids_are_frames() {
        assert!(is_dxgi_present_start(42));
        assert!(is_dxgi_present_start(55));
        assert!(is_d3d9_present_start(1));
    }

    #[test]
    fn opcode1_lookalikes_are_not_frames() {
        assert!(!is_dxgi_present_start(DXGI_SWAPCHAIN_START));
        assert!(!is_dxgi_present_start(DXGI_RESIZEBUFFERS_START));
        assert!(!is_dxgi_present_start(43)); // Present_Stop
    }
}
