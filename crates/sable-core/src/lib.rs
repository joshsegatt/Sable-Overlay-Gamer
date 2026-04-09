// sable-core: Shared types, IPC protocol, and data structures
// Used by all Sable crates. Zero platform-specific code here.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Game Entry ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GamePlatform {
    Steam,
    Epic,
    Gog,
    Ea,
    BattleNet,
    Ubisoft,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEntry {
    pub id: Uuid,
    pub name: String,
    pub exe_path: String,
    pub icon_path: Option<String>,
    pub platform: GamePlatform,
    pub last_played: Option<DateTime<Utc>>,
    pub install_size_mb: Option<u64>,
    pub preset_id: Option<Uuid>,
}

// ─── GPU Telemetry ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub vendor: GpuVendor,
    pub name: String,
    pub driver_version: String,
    pub vram_total_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GpuMetrics {
    pub gpu_usage_pct: Option<f32>,
    pub gpu_temp_c: Option<f32>,
    pub vram_used_mb: Option<u64>,
    pub vram_total_mb: Option<u64>,
    pub core_clock_mhz: Option<u32>,
    pub memory_clock_mhz: Option<u32>,
    pub power_draw_w: Option<f32>,
    pub power_limit_w: Option<f32>,
    pub fan_speed_pct: Option<f32>,
    pub is_power_limited: Option<bool>,
    pub is_thermal_limited: Option<bool>,
}

// ─── CPU Telemetry ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CpuMetrics {
    pub usage_pct: Option<f32>,
    pub temp_c: Option<f32>,
    pub frequency_mhz: Option<u32>,
    pub core_count: Option<u32>,
    pub logical_count: Option<u32>,
    pub name: Option<String>,
}

// ─── Frame Metrics ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrameMetrics {
    pub fps_avg: Option<f32>,
    pub fps_1_percent_low: Option<f32>,
    pub fps_0_1_percent_low: Option<f32>,
    pub frametime_avg_ms: Option<f32>,
    pub frametime_p99_ms: Option<f32>,
    pub frametime_stddev_ms: Option<f32>,
    /// Last N frame times in ms for sparkline rendering (capped at 360 for transfer)
    pub frametime_history: Vec<f32>,
    pub target_process: Option<String>,
}

// ─── System Telemetry Snapshot ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelemetrySnapshot {
    pub timestamp: Option<DateTime<Utc>>,
    pub gpu: GpuMetrics,
    pub cpu: CpuMetrics,
    pub frames: FrameMetrics,
    pub ram_used_mb: Option<u64>,
    pub ram_total_mb: Option<u64>,
}

// ─── Optimization Presets ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PresetChange {
    /// Switch Windows power plan by GUID-string
    SetPowerPlan { plan_guid: String, plan_name: String },
    /// Set process priority (0=Normal,1=AboveNormal,2=High — never Realtime)
    SetProcessPriority { priority: u8 },
    /// Pin game to specific CPU cores (bitmask)
    SetCpuAffinity { mask: u64 },
    /// Toggle Hardware Accelerated GPU Scheduling (requires advisory + info banner)
    SetHags { enabled: bool },
    /// Disable Xbox Game DVR / Background recording
    DisableGameDvr,
    /// Disable Xbox Game Bar
    DisableGameBar,
    /// Enable Windows Game Mode
    EnableGameMode,
    /// Reduce background process priorities during gaming session
    ThrottleBackgroundProcesses { threshold_cpu_pct: f32 },
    /// Vendor-specific NVIDIA DRS profile setting
    NvidiaDrsSetting { setting_id: u32, value: u32, description: String },
    /// Set Win32 CPU priority separation (foreground vs background quantum).
    /// Default Windows value: 2. Gaming-optimized value: 38 (0x26) — maximum
    /// foreground quanta with no background boost. Reversible registry write.
    SetPrioritySeparation { value: u32 },
    /// Stop the Windows Search Indexer (WSearch) service and set it to manual
    /// start so it doesn't restart on reboot. Eliminates disk/CPU overhead from
    /// background indexing during gameplay. Rollback restarts the service.
    DisableSearchIndexer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub risk: RiskLevel,
    pub changes: Vec<PresetChange>,
    pub is_bundled: bool,
    pub is_applied: bool,
}

// ─── Rollback Snapshot ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackSnapshot {
    pub id: Uuid,
    pub preset_id: Uuid,
    pub timestamp: DateTime<Utc>,
    /// Serialized "before" state for each change type
    pub state: serde_json::Value,
}

// ─── Benchmark Session ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetricsSet {
    pub fps_avg: f32,
    pub fps_1pct_low: f32,
    pub frametime_avg_ms: f32,
    pub frametime_p99_ms: f32,
    pub gpu_usage_pct: f32,
    pub cpu_usage_pct: f32,
    pub vram_used_mb: u64,
    pub gpu_temp_c: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSession {
    pub id: Uuid,
    pub game_id: Uuid,
    pub game_name: String,
    pub timestamp: DateTime<Utc>,
    pub duration_secs: u32,
    pub preset_applied: Option<Uuid>,
    pub before: Option<BenchmarkMetricsSet>,
    pub after: Option<BenchmarkMetricsSet>,
    pub label: Option<String>,
    pub frametime_history: Vec<f32>,
}

// ─── Bottleneck Diagnosis ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BottleneckKind {
    GpuBound,
    CpuBound,
    VramPressure,
    ThermalThrottle,
    PowerLimit,
    LowGpuAnomaly,
    BackgroundDrain,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleneckDiagnosis {
    pub kind: BottleneckKind,
    pub title: String,
    pub cause: String,
    pub recommendation: String,
    /// 0–100 confidence score
    pub confidence: u8,
}

// ─── System Info ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub gpu: Option<GpuInfo>,
    pub cpu_name: Option<String>,
    pub cpu_cores: Option<u32>,
    pub cpu_threads: Option<u32>,
    pub ram_total_mb: Option<u64>,
    pub os_version: Option<String>,
    pub hags_enabled: Option<bool>,
    pub power_plan_name: Option<String>,
    pub power_plan_guid: Option<String>,
}

// ─── IPC Protocol ────────────────────────────────────────────────────────────

/// Messages sent FROM the Tauri app (requests to the service)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceRequest {
    GetTelemetry,
    GetSystemInfo,
    GetGames,
    ApplyPreset { preset_id: Uuid },
    RollbackPreset { preset_id: Uuid },
    GetPresets,
    StartBenchmark { game_id: Uuid },
    StopBenchmark,
    GetBenchmarkSessions,
    GetBottleneckReport { session_id: Uuid },
    SetOverlayConfig(OverlayConfig),
    GetOverlayConfig,
    Ping,
}

/// Messages sent FROM the service (responses)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceResponse {
    Telemetry(TelemetrySnapshot),
    SystemInfo(SystemInfo),
    Games(Vec<GameEntry>),
    Presets(Vec<Preset>),
    BenchmarkSessions(Vec<BenchmarkSession>),
    BottleneckReport(Vec<BottleneckDiagnosis>),
    OverlayConfig(OverlayConfig),
    PresetApplied { preset_id: Uuid },
    PresetRolledBack { preset_id: Uuid },
    BenchmarkStarted,
    BenchmarkStopped,
    Pong,
    Error(String),
}

// ─── Overlay Config ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OverlayPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayConfig {
    pub enabled: bool,
    pub position: OverlayPosition,
    pub scale: f32,
    pub opacity: f32,
    pub show_fps: bool,
    pub show_frametime: bool,
    pub show_gpu_usage: bool,
    pub show_cpu_usage: bool,
    pub show_gpu_temp: bool,
    pub show_vram: bool,
    pub show_ram: bool,
    pub streamer_mode: bool,
    pub bg_fill: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            position: OverlayPosition::TopLeft,
            scale: 1.0,
            opacity: 0.85,
            show_fps: true,
            show_frametime: true,
            show_gpu_usage: true,
            show_cpu_usage: true,
            show_gpu_temp: true,
            show_vram: false,
            show_ram: false,
            streamer_mode: false,
            bg_fill: true,
        }
    }
}

// ─── App Settings ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub launch_on_startup: bool,
    pub start_minimized: bool,
    pub auto_apply_presets: bool,
    pub telemetry_interval_ms: u32,
    pub overlay: OverlayConfig,
    pub expert_mode: bool,
    pub theme: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            launch_on_startup: false,
            start_minimized: false,
            auto_apply_presets: false,
            telemetry_interval_ms: 1000,
            overlay: OverlayConfig::default(),
            expert_mode: false,
            theme: "dark".to_string(),
        }
    }
}
