// lib/api.ts — Tauri invoke wrappers
// All backend calls go through here. Returns typed responses.
// When the service is offline, returns empty/default data.

import { invoke } from '@tauri-apps/api/core';

// ─── Types (mirroring sable-core) ─────────────────────────────────────────────

export interface GpuMetrics {
  gpu_usage_pct: number | null;
  gpu_temp_c: number | null;
  vram_used_mb: number | null;
  vram_total_mb: number | null;
  core_clock_mhz: number | null;
  memory_clock_mhz: number | null;
  power_draw_w: number | null;
  power_limit_w: number | null;
  fan_speed_pct: number | null;
  is_power_limited: boolean | null;
  is_thermal_limited: boolean | null;
}

export interface CpuMetrics {
  usage_pct: number | null;
  temp_c: number | null;
  frequency_mhz: number | null;
  core_count: number | null;
  logical_count: number | null;
  name: string | null;
}

export interface FrameMetrics {
  fps_avg: number | null;
  fps_1_percent_low: number | null;
  fps_0_1_percent_low: number | null;
  frametime_avg_ms: number | null;
  frametime_p99_ms: number | null;
  frametime_stddev_ms: number | null;
  frametime_history: number[];
  target_process: string | null;
}

export interface TelemetrySnapshot {
  timestamp: string | null;
  gpu: GpuMetrics;
  cpu: CpuMetrics;
  frames: FrameMetrics;
  ram_used_mb: number | null;
  ram_total_mb: number | null;
}

export type GamePlatform = 'Steam' | 'Epic' | 'Gog' | 'Ea' | 'BattleNet' | 'Ubisoft' | 'Custom';

export interface GameEntry {
  id: string;
  name: string;
  exe_path: string;
  icon_path: string | null;
  platform: GamePlatform;
  last_played: string | null;
  install_size_mb: number | null;
  preset_id: string | null;
}

export type RiskLevel = 'Low' | 'Medium' | 'High';

export interface Preset {
  id: string;
  name: string;
  description: string;
  risk: RiskLevel;
  changes: unknown[];
  is_bundled: boolean;
  is_applied: boolean;
}

export type BottleneckKind =
  | 'GpuBound' | 'CpuBound' | 'VramPressure'
  | 'ThermalThrottle' | 'PowerLimit' | 'LowGpuAnomaly'
  | 'BackgroundDrain' | 'None';

export interface BottleneckDiagnosis {
  kind: BottleneckKind;
  title: string;
  cause: string;
  recommendation: string;
  confidence: number;
}

export interface GpuInfo {
  vendor: 'Nvidia' | 'Amd' | 'Intel' | 'Unknown';
  name: string;
  driver_version: string;
  vram_total_mb: number;
}

export interface SystemInfo {
  gpu: GpuInfo | null;
  cpu_name: string | null;
  cpu_cores: number | null;
  cpu_threads: number | null;
  ram_total_mb: number | null;
  os_version: string | null;
  hags_enabled: boolean | null;
  power_plan_name: string | null;
  power_plan_guid: string | null;
}

export interface BenchmarkMetricsSet {
  fps_avg: number;
  fps_1pct_low: number;
  frametime_avg_ms: number;
  frametime_p99_ms: number;
  gpu_usage_pct: number;
  cpu_usage_pct: number;
  vram_used_mb: number;
  gpu_temp_c: number;
}

export interface BenchmarkSession {
  id: string;
  game_id: string;
  game_name: string;
  timestamp: string;
  duration_secs: number;
  preset_applied: string | null;
  before: BenchmarkMetricsSet | null;
  after: BenchmarkMetricsSet | null;
  label: string | null;
  frametime_history: number[];
}

export type OverlayPosition = 'TopLeft' | 'TopRight' | 'BottomLeft' | 'BottomRight';

export interface OverlayConfig {
  enabled: boolean;
  position: OverlayPosition;
  scale: number;
  opacity: number;
  show_fps: boolean;
  show_frametime: boolean;
  show_gpu_usage: boolean;
  show_cpu_usage: boolean;
  show_gpu_temp: boolean;
  show_vram: boolean;
  show_ram: boolean;
  streamer_mode: boolean;
  bg_fill: boolean;
}

export interface AppSettings {
  launch_on_startup: boolean;
  start_minimized: boolean;
  auto_apply_presets: boolean;
  telemetry_interval_ms: number;
  overlay: OverlayConfig;
  expert_mode: boolean;
  theme: string;
}

// ─── Default Values ───────────────────────────────────────────────────────────

export const defaultTelemetry = (): TelemetrySnapshot => ({
  timestamp: null,
  gpu: {
    gpu_usage_pct: null, gpu_temp_c: null, vram_used_mb: null, vram_total_mb: null,
    core_clock_mhz: null, memory_clock_mhz: null, power_draw_w: null,
    power_limit_w: null, fan_speed_pct: null, is_power_limited: null, is_thermal_limited: null,
  },
  cpu: { usage_pct: null, temp_c: null, frequency_mhz: null, core_count: null, logical_count: null, name: null },
  frames: {
    fps_avg: null, fps_1_percent_low: null, fps_0_1_percent_low: null,
    frametime_avg_ms: null, frametime_p99_ms: null, frametime_stddev_ms: null,
    frametime_history: [], target_process: null,
  },
  ram_used_mb: null,
  ram_total_mb: null,
});

export const defaultOverlayConfig = (): OverlayConfig => ({
  enabled: true,
  position: 'TopLeft',
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
});

export const defaultSettings = (): AppSettings => ({
  launch_on_startup: false,
  start_minimized: false,
  auto_apply_presets: false,
  telemetry_interval_ms: 1000,
  overlay: defaultOverlayConfig(),
  expert_mode: false,
  theme: 'dark',
});

// ─── API Functions ────────────────────────────────────────────────────────────

async function safeInvoke<T>(command: string, args?: Record<string, unknown>, fallback?: T): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    console.warn(`[sable] ${command} failed:`, e);
    if (fallback !== undefined) return fallback;
    throw e;
  }
}

export const api = {
  ping: () => safeInvoke<boolean>('cmd_ping', {}, false),

  getTelemetry: () =>
    safeInvoke<TelemetrySnapshot>('cmd_get_telemetry', {}, defaultTelemetry() as unknown as TelemetrySnapshot),

  getSystemInfo: () =>
    safeInvoke<SystemInfo>('cmd_get_system_info', {}, { gpu: null, cpu_name: null, cpu_cores: null, cpu_threads: null, ram_total_mb: null, os_version: null, hags_enabled: null, power_plan_name: null, power_plan_guid: null }),

  getGames: () =>
    safeInvoke<GameEntry[]>('cmd_get_games', {}, []),

  getPresets: () =>
    safeInvoke<Preset[]>('cmd_get_presets', {}, []),

  applyPreset: (presetId: string) =>
    safeInvoke<unknown>('cmd_apply_preset', { preset_id: presetId }),

  rollbackPreset: (presetId: string) =>
    safeInvoke<unknown>('cmd_rollback_preset', { preset_id: presetId }),

  getBenchmarkSessions: () =>
    safeInvoke<BenchmarkSession[]>('cmd_get_benchmark_sessions', {}, []),

  startBenchmark: (gameId: string) =>
    safeInvoke<unknown>('cmd_start_benchmark', { game_id: gameId }),

  stopBenchmark: () =>
    safeInvoke<unknown>('cmd_stop_benchmark'),

  getBottleneckReport: (sessionId: string) =>
    safeInvoke<BottleneckDiagnosis[]>('cmd_get_bottleneck_report', { session_id: sessionId }, []),

  getOverlayConfig: () =>
    safeInvoke<OverlayConfig>('cmd_get_overlay_config', {}, defaultOverlayConfig()),

  setOverlayConfig: (config: OverlayConfig) =>
    safeInvoke<unknown>('cmd_set_overlay_config', { config }),

  setOverlayVisible: (enabled: boolean) =>
    safeInvoke<void>('cmd_set_overlay_visible', { enabled }),

  launchService: () =>
    safeInvoke<void>('cmd_launch_service'),

  getSettings: () =>
    safeInvoke<AppSettings>('cmd_get_settings', {}, defaultSettings()),

  saveSettings: (settings: AppSettings) =>
    safeInvoke<void>('cmd_save_settings', { settings }),

  checkForUpdate: async (): Promise<{ available: boolean; version?: string; body?: string }> => {
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const update = await check();
      if (update) {
        return { available: true, version: update.version, body: update.body ?? undefined };
      }
      return { available: false };
    } catch {
      return { available: false };
    }
  },
} as const;
