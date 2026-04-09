import type { Preset } from '../lib/api';
import { RiskBadge } from './RiskBadge';
import { StatusBadge } from './StatusBadge';
import s from './PresetCard.module.css';

interface PresetCardProps {
  preset: Preset;
  applied: boolean;
  onApply: () => void;
  onRollback: () => void;
  isApplying?: boolean;
  isRollingBack?: boolean;
}

export function PresetCard({
  preset,
  applied,
  onApply,
  onRollback,
  isApplying = false,
  isRollingBack = false,
}: PresetCardProps) {
  return (
    <article className={`${s.card} ${applied ? s.isApplied : ''}`}>
      <div className={s.header}>
        <div className={s.titleRow}>
          <span className={s.name}>{preset.name}</span>
          <span className={s.description}>{preset.description}</span>
        </div>
        <RiskBadge risk={preset.risk} />
      </div>

      <span className={s.changes}>
        {preset.changes.length} change{preset.changes.length !== 1 ? 's' : ''}
      </span>

      <div className={s.footer}>
        <StatusBadge status={applied ? 'applied' : 'not-applied'} />
        <div className={s.actions}>
          {applied && (
            <button
              className={`${s.btn} ${s.btnRollback}`}
              onClick={onRollback}
              disabled={isRollingBack}
            >
              {isRollingBack ? '…' : 'Rollback'}
            </button>
          )}
          {!applied && (
            <button
              className={`${s.btn} ${s.btnApply}`}
              onClick={onApply}
              disabled={isApplying}
            >
              {isApplying ? '…' : 'Apply'}
            </button>
          )}
        </div>
      </div>
    </article>
  );
}
