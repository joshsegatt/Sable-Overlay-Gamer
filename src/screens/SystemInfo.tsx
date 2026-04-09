import { useEffect, useState } from 'react';
import { useAppStore } from '../stores/appStore';
import { MetricRow } from '../components/MetricRow';
import { StatusBadge } from '../components/StatusBadge';
import { api } from '../lib/api';
import s from './shared.module.css';

export function SystemInfo() {
  const { systemInfo, fetchSystemInfo, serviceOnline, checkService } = useAppStore();
  const [launching, setLaunching] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);

  useEffect(() => {
    checkService();
    fetchSystemInfo();
  }, [checkService, fetchSystemInfo]);

  const handleLaunchService = async () => {
    setLaunching(true);
    setLaunchError(null);
    try {
      await api.launchService();
      // Re-check after a short delay to allow the service to start
      setTimeout(() => { checkService(); fetchSystemInfo(); }, 2500);
    } catch (e) {
      setLaunchError(String(e));
    } finally {
      setLaunching(false);
    }
  };

  const info = systemInfo;

  return (
    <main className={s.page}>
      <div className={s.header}>
        <div className={s.headerLeft}>
          <h1 className={s.pageTitle}>System</h1>
          <p className={s.pageSubtitle}>Hardware and OS configuration</p>
        </div>
        <StatusBadge status={serviceOnline ? 'online' : 'offline'} />
      </div>

      {!serviceOnline && (
        <div className={s.card} style={{ borderColor: 'rgba(245,166,35,0.35)', background: 'rgba(245,166,35,0.06)' }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 16 }}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              <p style={{ fontSize: 'var(--text-sm)', color: 'var(--color-warning)', fontWeight: 500 }}>
                ⚠ Sable service is not running
              </p>
              <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)' }}>
                The service runs elevated to collect hardware data and apply optimizations.
              </p>
              {launchError && (
                <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-danger)', marginTop: 2 }}>{launchError}</p>
              )}
            </div>
            <button className={s.btnPrimary} onClick={handleLaunchService} disabled={launching} style={{ flexShrink: 0 }}>
              {launching ? 'Starting…' : 'Start Service'}
            </button>
          </div>
        </div>
      )}

      {info ? (
        <>
          <div className={s.section}>
            <span className={s.sectionTitle}>GPU</span>
            <div className={s.card}>
              <MetricRow label="Name" value={info.gpu?.name ?? '—'} />
              <MetricRow label="Vendor" value={info.gpu?.vendor ?? '—'} />
              <MetricRow label="Driver" value={info.gpu?.driver_version ?? '—'} />
              <MetricRow label="VRAM" value={info.gpu ? `${(info.gpu.vram_total_mb / 1024).toFixed(1)} GB` : '—'} />
            </div>
          </div>

          <div className={s.section}>
            <span className={s.sectionTitle}>CPU</span>
            <div className={s.card}>
              <MetricRow label="Name" value={info.cpu_name ?? '—'} />
              <MetricRow label="Cores / Threads" value={info.cpu_cores && info.cpu_threads ? `${info.cpu_cores} / ${info.cpu_threads}` : '—'} />
            </div>
          </div>

          <div className={s.section}>
            <span className={s.sectionTitle}>Memory</span>
            <div className={s.card}>
              <MetricRow label="RAM" value={info.ram_total_mb ? `${(info.ram_total_mb / 1024).toFixed(0)} GB` : '—'} />
            </div>
          </div>

          <div className={s.section}>
            <span className={s.sectionTitle}>Windows</span>
            <div className={s.card}>
              <MetricRow label="OS Version" value={info.os_version ?? '—'} />
              <MetricRow label="HAGS" value={info.hags_enabled === null ? '—' : info.hags_enabled ? 'Enabled' : 'Disabled'} />
              <MetricRow label="Power Plan" value={info.power_plan_name ?? '—'} />
            </div>
          </div>
        </>
      ) : (
        <div className={s.emptyState}>
          <span className={s.emptyIcon}>🖥</span>
          <span className={s.emptyText}>
            {serviceOnline ? 'Loading…' : 'Start the Sable service to view system info.'}
          </span>
        </div>
      )}
    </main>
  );
}
