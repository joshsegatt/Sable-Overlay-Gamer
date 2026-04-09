import type { OverlayConfig } from '../lib/api';
import s from './OverlayPreview.module.css';

interface OverlayPreviewProps {
  config: OverlayConfig;
}

function colorClass(value: number, warnAt: number, badAt: number): string {
  if (value >= badAt)  return s.red;
  if (value >= warnAt) return s.amber;
  return s.green;
}

export function OverlayPreview({ config }: OverlayPreviewProps) {
  // Static representative values for preview
  const fps     = 144;
  const p1      = 118;
  const gpuUsage = 82;
  const cpuUsage = 45;
  const gpuTemp  = 72;
  const vramUsed = 7.2;
  const frametime = 6.9;

  return (
    <div className={s.preview}>
      {config.show_fps && (
        <div className={s.line}>
          <span className={s.label}>FPS</span>
          <span className={`${s.value} ${colorClass(fps, 45, 30)}`}>{fps}</span>
          <span className={`${s.value} ${s.white}`}>/ {p1}</span>
        </div>
      )}
      {config.show_frametime && (
        <div className={s.line}>
          <span className={s.label}>FT </span>
          <span className={`${s.value} ${colorClass(100 - frametime * 3, 50, 30)}`}>{frametime.toFixed(1)}ms</span>
        </div>
      )}
      {config.show_gpu_usage && (
        <div className={s.line}>
          <span className={s.label}>GPU</span>
          <span className={`${s.value} ${colorClass(100 - gpuUsage, 20, 5)}`}>{gpuUsage}%</span>
        </div>
      )}
      {config.show_cpu_usage && (
        <div className={s.line}>
          <span className={s.label}>CPU</span>
          <span className={`${s.value} ${colorClass(100 - cpuUsage, 20, 5)}`}>{cpuUsage}%</span>
        </div>
      )}
      {config.show_gpu_temp && (
        <div className={s.line}>
          <span className={s.label}>TMP</span>
          <span className={`${s.value} ${colorClass(100 - gpuTemp, 17, 7)}`}>{gpuTemp}°C</span>
        </div>
      )}
      {config.show_vram && (
        <div className={s.line}>
          <span className={s.label}>VRM</span>
          <span className={`${s.value} ${s.white}`}>{vramUsed.toFixed(1)}G</span>
        </div>
      )}
    </div>
  );
}
