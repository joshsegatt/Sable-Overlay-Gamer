import { useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useGamesStore } from '../stores/gamesStore';
import { usePresetsStore } from '../stores/presetsStore';
import { PresetCard } from '../components/PresetCard';
import { useToast } from '../components/ToastNotification';
import s from './shared.module.css';

export function GameProfile() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const games = useGamesStore(st => st.games);
  const game = games.find(g => g.id === id);
  const { presets, apply, rollback, applyingId, rollingBackId } = usePresetsStore();
  const { toast } = useToast();

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

  if (!game) {
    return (
      <main className={s.page}>
        <div className={s.emptyState}>
          <span className={s.emptyIcon}>❓</span>
          <span className={s.emptyText}>Game not found.</span>
          <button className={s.btnGhost} onClick={() => navigate('/games')}>← Back to Games</button>
        </div>
      </main>
    );
  }

  return (
    <main className={s.page}>
      <div className={s.header}>
        <div className={s.headerLeft}>
          <button
            style={{ background: 'none', border: 'none', color: 'var(--color-text-muted)', cursor: 'pointer', fontSize: 'var(--text-sm)', marginBottom: 4, padding: 0 }}
            onClick={() => navigate('/games')}
          >
            ← Games
          </button>
          <h1 className={s.pageTitle}>{game.name}</h1>
          <p className={s.pageSubtitle}>{game.platform} · {game.exe_path}</p>
        </div>
      </div>

      <div className={s.section}>
        <span className={s.sectionTitle}>Optimization presets</span>
        <p style={{ fontSize: 'var(--text-sm)', color: 'var(--color-text-muted)' }}>
          Apply any preset to optimize your system for this game. All changes are reversible.
        </p>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          {presets.map(preset => (
            <PresetCard
              key={preset.id}
              preset={preset}
              applied={preset.is_applied}
              onApply={() => handleApply(preset.id, preset.name)}
              onRollback={() => handleRollback(preset.id, preset.name)}
              isApplying={applyingId === preset.id}
              isRollingBack={rollingBackId === preset.id}
            />
          ))}
        </div>
      </div>
    </main>
  );
}
