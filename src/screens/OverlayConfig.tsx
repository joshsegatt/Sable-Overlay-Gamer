import { useState } from 'react';
import { useAppStore } from '../stores/appStore';
import { OverlayPreview } from '../components/OverlayPreview';
import type { OverlayConfig, OverlayPosition } from '../lib/api';
import s from './shared.module.css';

function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 0', borderBottom: '1px solid var(--color-border)', cursor: 'pointer' }}>
      <span style={{ fontSize: 'var(--text-sm)', color: 'var(--color-text-secondary)' }}>{label}</span>
      <div
        role="switch"
        aria-checked={checked}
        style={{
          width: 36, height: 20, borderRadius: 10,
          background: checked ? 'var(--color-accent)' : 'var(--color-border)',
          position: 'relative', transition: 'background 150ms',
          cursor: 'pointer',
        }}
        onClick={() => onChange(!checked)}
      >
        <div style={{
          position: 'absolute', top: 2, left: checked ? 18 : 2, width: 16, height: 16,
          borderRadius: 8, background: '#fff', transition: 'left 150ms',
        }} />
      </div>
    </label>
  );
}

const positions: OverlayPosition[] = ['TopLeft', 'TopRight', 'BottomLeft', 'BottomRight'];

export function OverlayConfig() {
  const { settings, saveSettings } = useAppStore();
  const [cfg, setCfg] = useState<import('../lib/api').OverlayConfig>(settings.overlay);
  const [saving, setSaving] = useState(false);

  const update = (patch: Partial<import('../lib/api').OverlayConfig>) => {
    setCfg(prev => ({ ...prev, ...patch }));
  };

  const save = async () => {
    setSaving(true);
    await saveSettings({ ...settings, overlay: cfg });
    setSaving(false);
  };

  return (
    <main className={s.page}>
      <div className={s.header}>
        <div className={s.headerLeft}>
          <h1 className={s.pageTitle}>Overlay</h1>
          <p className={s.pageSubtitle}>Configure the in-game metric overlay.</p>
        </div>
        <button className={s.btnPrimary} onClick={save} disabled={saving}>
          {saving ? 'Saving…' : 'Save changes'}
        </button>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 260px', gap: 24 }}>
        {/* Settings column */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
          <div className={s.card}>
            <Toggle label="Enable overlay" checked={cfg.enabled} onChange={v => update({ enabled: v })} />
            <Toggle label="Background fill" checked={cfg.bg_fill} onChange={v => update({ bg_fill: v })} />
            <Toggle label="Streamer mode (hide values)" checked={cfg.streamer_mode} onChange={v => update({ streamer_mode: v })} />
          </div>

          <div className={s.section}>
            <span className={s.sectionTitle}>Metrics to show</span>
            <div className={s.card}>
              <Toggle label="FPS" checked={cfg.show_fps} onChange={v => update({ show_fps: v })} />
              <Toggle label="Frametime" checked={cfg.show_frametime} onChange={v => update({ show_frametime: v })} />
              <Toggle label="GPU Usage" checked={cfg.show_gpu_usage} onChange={v => update({ show_gpu_usage: v })} />
              <Toggle label="CPU Usage" checked={cfg.show_cpu_usage} onChange={v => update({ show_cpu_usage: v })} />
              <Toggle label="GPU Temperature" checked={cfg.show_gpu_temp} onChange={v => update({ show_gpu_temp: v })} />
              <Toggle label="VRAM" checked={cfg.show_vram} onChange={v => update({ show_vram: v })} />
              <Toggle label="RAM" checked={cfg.show_ram} onChange={v => update({ show_ram: v })} />
            </div>
          </div>

          <div className={s.section}>
            <span className={s.sectionTitle}>Position</span>
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              {positions.map(pos => (
                <button
                  key={pos}
                  style={{
                    padding: '6px 14px',
                    borderRadius: 'var(--radius-sm)',
                    fontSize: 'var(--text-xs)',
                    fontWeight: 500,
                    border: `1px solid ${cfg.position === pos ? 'var(--color-accent)' : 'var(--color-border)'}`,
                    background: cfg.position === pos ? 'rgba(79,142,247,0.12)' : 'transparent',
                    color: cfg.position === pos ? 'var(--color-accent)' : 'var(--color-text-secondary)',
                    cursor: 'pointer',
                  }}
                  onClick={() => update({ position: pos })}
                >
                  {pos.replace(/([A-Z])/g, ' $1').trim()}
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* Preview column */}
        <div className={s.section} style={{ position: 'sticky', top: 0, alignSelf: 'start' }}>
          <span className={s.sectionTitle}>Preview</span>
          <div style={{
            background: 'var(--color-bg-elevated)',
            border: '1px solid var(--color-border)',
            borderRadius: 'var(--radius-md)',
            padding: 20,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            minHeight: 160,
          }}>
            {cfg.enabled ? (
              <OverlayPreview config={cfg} />
            ) : (
              <span style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)' }}>Overlay disabled</span>
            )}
          </div>
        </div>
      </div>
    </main>
  );
}
