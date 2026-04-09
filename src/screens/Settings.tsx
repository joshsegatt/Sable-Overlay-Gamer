import { useCallback, useState } from 'react';
import { useAppStore } from '../stores/appStore';
import { api } from '../lib/api';
import type { AppSettings } from '../lib/api';
import s from './shared.module.css';

// Keep in sync with package.json / tauri.conf.json
const APP_VERSION = '0.1.0';

function Toggle({ label, description, checked, onChange }: {
  label: string; description?: string; checked: boolean; onChange: (v: boolean) => void;
}) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '10px 0', borderBottom: '1px solid var(--color-border)' }}>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
        <span style={{ fontSize: 'var(--text-sm)', color: 'var(--color-text-primary)', fontWeight: 500 }}>{label}</span>
        {description && <span style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)' }}>{description}</span>}
      </div>
      <div
        role="switch"
        aria-checked={checked}
        style={{
          width: 36, height: 20, borderRadius: 10, flexShrink: 0,
          background: checked ? 'var(--color-accent)' : 'var(--color-border)',
          position: 'relative', transition: 'background 150ms', cursor: 'pointer',
        }}
        onClick={() => onChange(!checked)}
      >
        <div style={{
          position: 'absolute', top: 2, left: checked ? 18 : 2, width: 16, height: 16,
          borderRadius: 8, background: '#fff', transition: 'left 150ms',
        }} />
      </div>
    </div>
  );
}

export function Settings() {
  const { settings, saveSettings, telemetryConsent, acceptTelemetryConsent } = useAppStore();
  const [local, setLocal] = useState<AppSettings>(settings);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [updateStatus, setUpdateStatus] = useState<string | null>(null);

  const update = (patch: Partial<AppSettings>) => {
    setLocal(prev => ({ ...prev, ...patch }));
    setSaved(false);
  };

  const save = async () => {
    setSaving(true);
    await saveSettings(local);
    setSaving(false);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const handleCheckUpdate = useCallback(async () => {
    setCheckingUpdate(true);
    setUpdateStatus(null);
    const result = await api.checkForUpdate();
    setCheckingUpdate(false);
    if (result.available) {
      setUpdateStatus(`v${result.version} available — restart to install`);
    } else {
      setUpdateStatus('Sable is up to date');
    }
  }, []);

  return (
    <main className={s.page}>
      <div className={s.header}>
        <div className={s.headerLeft}>
          <h1 className={s.pageTitle}>Settings</h1>
          <p className={s.pageSubtitle}>App preferences and behavior</p>
        </div>
        <button className={s.btnPrimary} onClick={save} disabled={saving}>
          {saving ? 'Saving…' : saved ? '✓ Saved' : 'Save'}
        </button>
      </div>

      <div className={s.section}>
        <span className={s.sectionTitle}>General</span>
        <div className={s.card}>
          <Toggle
            label="Launch on startup"
            description="Start Sable automatically when Windows boots"
            checked={local.launch_on_startup}
            onChange={v => update({ launch_on_startup: v })}
          />
          <Toggle
            label="Start minimized"
            description="Open Sable to the system tray on launch"
            checked={local.start_minimized}
            onChange={v => update({ start_minimized: v })}
          />
          <Toggle
            label="Auto-apply presets"
            description="Automatically apply the last used preset when a game is detected"
            checked={local.auto_apply_presets}
            onChange={v => update({ auto_apply_presets: v })}
          />
        </div>
      </div>

      <div className={s.section}>
        <span className={s.sectionTitle}>Advanced</span>
        <div className={s.card}>
          <Toggle
            label="Expert mode"
            description="Show additional technical details and unsafe preset options"
            checked={local.expert_mode}
            onChange={v => update({ expert_mode: v })}
          />
        </div>
      </div>

      <div className={s.section}>
        <span className={s.sectionTitle}>Telemetry</span>
        <div className={s.card} style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <div>
              <p style={{ fontSize: 'var(--text-sm)', fontWeight: 500, color: 'var(--color-text-primary)' }}>Polling interval</p>
              <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)' }}>How often to sample GPU/CPU metrics (ms)</p>
            </div>
            <select
              value={local.telemetry_interval_ms}
              onChange={e => update({ telemetry_interval_ms: Number(e.target.value) })}
              style={{
                padding: '6px 10px',
                background: 'var(--color-bg-elevated)',
                border: '1px solid var(--color-border)',
                borderRadius: 'var(--radius-sm)',
                color: 'var(--color-text-primary)',
                fontSize: 'var(--text-sm)',
                cursor: 'pointer',
              }}
            >
              <option value={500}>500 ms</option>
              <option value={1000}>1000 ms</option>
              <option value={2000}>2000 ms</option>
            </select>
          </div>
        </div>
      </div>

      <div className={s.section}>
        <span className={s.sectionTitle}>Privacy</span>
        <div className={s.card} style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <div>
              <p style={{ fontSize: 'var(--text-sm)', fontWeight: 500, color: 'var(--color-text-primary)', margin: 0 }}>
                On-device telemetry collection
              </p>
              <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', margin: '2px 0 0' }}>
                GPU, CPU, and frametime data — never sent to external servers
              </p>
            </div>
            <span style={{
              fontSize: 11,
              fontWeight: 600,
              letterSpacing: '0.06em',
              padding: '3px 9px',
              borderRadius: 999,
              color: telemetryConsent ? 'var(--color-positive)' : 'var(--color-text-muted)',
              background: telemetryConsent ? 'rgba(61,214,140,0.1)' : 'rgba(74,82,98,0.18)',
              border: `1px solid ${telemetryConsent ? 'rgba(61,214,140,0.3)' : 'var(--color-border)'}`,
            }}>
              {telemetryConsent ? 'CONSENTED' : 'NOT SET'}
            </span>
          </div>
          {!telemetryConsent && (
            <button
              className={s.btnPrimary}
              style={{ alignSelf: 'flex-start', fontSize: 'var(--text-xs)', padding: '6px 14px' }}
              onClick={acceptTelemetryConsent}
            >
              Accept data policy
            </button>
          )}
          {telemetryConsent && (
            <button
              className={s.btnGhost}
              style={{ alignSelf: 'flex-start', fontSize: 'var(--text-xs)', padding: '6px 14px' }}
              onClick={() => {
                localStorage.removeItem('sable_consent');
                window.location.reload();
              }}
            >
              Revoke consent
            </button>
          )}
        </div>
      </div>

      <div className={s.section}>
        <span className={s.sectionTitle}>Updates</span>
        <div className={s.card} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 16 }}>
          <div>
            <p style={{ fontSize: 'var(--text-sm)', fontWeight: 500, color: 'var(--color-text-primary)', margin: 0 }}>Sable v{APP_VERSION}</p>
            <p style={{ fontSize: 'var(--text-xs)', color: updateStatus?.includes('available') ? 'var(--color-positive)' : 'var(--color-text-muted)', margin: '2px 0 0' }}>
              {updateStatus ?? 'Check for the latest version'}
            </p>
          </div>
          <button
            className={s.btnGhost}
            onClick={handleCheckUpdate}
            disabled={checkingUpdate}
            style={{ fontSize: 'var(--text-xs)', padding: '7px 16px', flexShrink: 0 }}
          >
            {checkingUpdate ? 'Checking…' : 'Check for updates'}
          </button>
        </div>
      </div>
    </main>
  );
}
