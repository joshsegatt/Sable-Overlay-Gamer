import type { OverlayConfig } from '../lib/api';
import s from './OverlayPreview.module.css';

interface OverlayPreviewProps {
  config: OverlayConfig;
}

function tone(kind: 'fps' | 'usage' | 'temp', value: number): string {
  if (kind === 'fps') {
    if (value >= 100) return s.ok;
    if (value >= 60) return s.ok;
    if (value >= 30) return s.warn;
    return s.bad;
  }
  if (kind === 'usage') {
    if (value >= 92) return s.bad;
    if (value >= 80) return s.warn;
    return s.ok;
  }
  if (value >= 85) return s.bad;
  if (value >= 75) return s.warn;
  return s.ok;
}

const SPARK = [7.1, 6.8, 6.9, 7.4, 6.6, 8.2, 6.9, 7.0, 9.1, 6.8, 6.7, 7.2, 6.9, 7.1, 6.8, 7.0, 7.3, 6.9];

export function OverlayPreview({ config }: OverlayPreviewProps) {
  const fps = 144;
  const low = 118;
  const ft = 6.9;
  const gpu = 82;
  const cpu = 45;
  const temp = 72;
  const vram = 7.2;
  const vramTotal = 12;
  const ram = 18.4;

  return (
    <div className={s.stage}>
      <div className={s.hud} style={{ opacity: config.opacity, transform: `scale(${config.scale})`, transformOrigin: 'top left' }}>
        <div className={s.header}>
          <span className={s.brand}>SABLE</span>
          <span className={s.process}>r5apex.exe</span>
        </div>
        {config.show_fps && (
          <div className={s.row}>
            <span className={s.label}>FPS</span>
            <span className={`${s.value} ${tone('fps', fps)}`}>
              {fps}
              <span className={s.sub}>{low}</span>
            </span>
          </div>
        )}
        {config.show_frametime && (
          <div className={s.row}>
            <span className={s.label}>FT</span>
            <span className={s.value}>{ft.toFixed(1)}<span className={s.sub}>ms</span></span>
          </div>
        )}
        {config.show_gpu_usage && (
          <div className={s.row}>
            <span className={s.label}>GPU</span>
            <span className={`${s.value} ${tone('usage', gpu)}`}>{gpu}%</span>
          </div>
        )}
        {config.show_cpu_usage && (
          <div className={s.row}>
            <span className={s.label}>CPU</span>
            <span className={`${s.value} ${tone('usage', cpu)}`}>{cpu}%</span>
          </div>
        )}
        {config.show_gpu_temp && !config.streamer_mode && (
          <div className={s.row}>
            <span className={s.label}>GPU °C</span>
            <span className={`${s.value} ${tone('temp', temp)}`}>{temp}</span>
          </div>
        )}
        {config.show_vram && (
          <div className={s.row}>
            <span className={s.label}>VRAM</span>
            <span className={s.value}>{vram.toFixed(1)}<span className={s.sub}>/{vramTotal}</span></span>
          </div>
        )}
        {config.show_ram && (
          <div className={s.row}>
            <span className={s.label}>RAM</span>
            <span className={s.value}>{ram.toFixed(1)}<span className={s.sub}>G</span></span>
          </div>
        )}
        {config.show_frametime && (
          <div className={s.spark} aria-hidden>
            {SPARK.map((ms, i) => (
              <span
                key={i}
                className={s.bar}
                style={{ height: `${Math.max(18, Math.min(100, (ms / 16.7) * 100))}%` }}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
