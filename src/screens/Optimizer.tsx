import { useCallback, useEffect, useState } from 'react';
import { usePresetsStore } from '../stores/presetsStore';
import { useAppStore } from '../stores/appStore';
import { PresetCard } from '../components/PresetCard';
import { StatusBadge } from '../components/StatusBadge';
import { useToast } from '../components/ToastNotification';
import { api } from '../lib/api';
import s from './shared.module.css';

export function Optimizer() {
  const { presets, fetch, apply, rollback, applyingId, rollingBackId } = usePresetsStore();
  const { serviceOnline, checkService } = useAppStore();
  const { toast } = useToast();
  const [launching, setLaunching] = useState(false);

  useEffect(() => { fetch(); }, [fetch]);

  const handleLaunchService = useCallback(async () => {
    setLaunching(true);
    try {
      await api.launchService();
      setTimeout(async () => {
        await checkService();
        setLaunching(false);
      }, 2500);
    } catch {
      toast('Could not start the service. Try running as administrator.', 'error');
      setLaunching(false);
    }
  }, [checkService, toast]);

  const handleApply = useCallback(async (presetId: string, presetName: string) => {
    try {
      await apply(presetId);
      toast(`"${presetName}" applied — rollback available`, 'success');
    } catch {
      toast(`Failed to apply "${presetName}"`, 'error');
    }
  }, [apply, toast]);

  const handleRollback = useCallback(async (presetId: string, presetName: string) => {
    try {
      await rollback(presetId);
      toast(`"${presetName}" rolled back`, 'info');
    } catch {
      toast(`Failed to roll back "${presetName}"`, 'error');
    }
  }, [rollback, toast]);

  const applied = presets.filter(p => p.is_applied);
  const available = presets.filter(p => !p.is_applied);

  return (
    <main className={s.page}>
      <div className={s.header}>
        <div className={s.headerLeft}>
          <h1 className={s.pageTitle}>Optimizer</h1>
          <p className={s.pageSubtitle}>One-click presets with full rollback. Every change is reversible.</p>
        </div>
        <StatusBadge status={serviceOnline ? 'online' : 'offline'} />
      </div>

      {!serviceOnline && (
        <div className={s.card} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 16, borderColor: 'rgba(245,166,35,0.35)', background: 'rgba(245,166,35,0.05)' }}>
          <div>
            <p style={{ fontSize: 'var(--text-sm)', fontWeight: 600, color: 'var(--color-warning)', margin: 0 }}>⚠ Service offline</p>
            <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', margin: '2px 0 0' }}>Presets cannot be applied until the Sable service is running.</p>
          </div>
          <button
            className={s.btnPrimary}
            onClick={handleLaunchService}
            disabled={launching}
            style={{ flexShrink: 0, fontSize: 'var(--text-xs)', padding: '7px 16px' }}
          >
            {launching ? 'Starting…' : 'Start Service'}
          </button>
        </div>
      )}

      {applied.length > 0 && (
        <div className={s.section}>
          <span className={s.sectionTitle}>Applied ({applied.length})</span>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            {applied.map(p => (
              <PresetCard
                key={p.id}
                preset={p}
                applied
                onApply={() => handleApply(p.id, p.name)}
                onRollback={() => handleRollback(p.id, p.name)}
                isApplying={applyingId === p.id}
                isRollingBack={rollingBackId === p.id}
              />
            ))}
          </div>
        </div>
      )}

      <div className={s.section}>
        <span className={s.sectionTitle}>Available ({available.length})</span>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          {available.map(p => (
            <PresetCard
              key={p.id}
              preset={p}
              applied={false}
              onApply={() => handleApply(p.id, p.name)}
              onRollback={() => handleRollback(p.id, p.name)}
              isApplying={applyingId === p.id}
              isRollingBack={rollingBackId === p.id}
            />
          ))}
        </div>
      </div>

      {presets.length === 0 && (
        <div className={s.emptyState}>
          <span className={s.emptyIcon}>⚡</span>
          <span className={s.emptyText}>No presets found. Make sure the service is running.</span>
        </div>
      )}
    </main>
  );
}
