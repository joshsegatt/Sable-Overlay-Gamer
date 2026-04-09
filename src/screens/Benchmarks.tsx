import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useBenchmarkStore } from '../stores/benchmarkStore';
import s from './shared.module.css';

function formatDuration(durationSecs: number): string {
  return durationSecs < 60 ? `${durationSecs}s` : `${Math.floor(durationSecs / 60)}m ${durationSecs % 60}s`;
}

export function Benchmarks() {
  const navigate = useNavigate();
  const { sessions, isBenchmarking, fetch, startBenchmark, stopBenchmark } = useBenchmarkStore();

  useEffect(() => { fetch(); }, [fetch]);

  return (
    <main className={s.page}>
      <div className={s.header}>
        <div className={s.headerLeft}>
          <h1 className={s.pageTitle}>Benchmarks</h1>
          <p className={s.pageSubtitle}>{sessions.length} session{sessions.length !== 1 ? 's' : ''} recorded</p>
        </div>
        <button
          className={isBenchmarking ? s.btnGhost : s.btnPrimary}
          onClick={isBenchmarking ? stopBenchmark : () => startBenchmark('')}
        >
          {isBenchmarking ? '⏹ Stop' : '⏺ Record'}
        </button>
      </div>

      {sessions.length === 0 && (
        <div className={s.emptyState}>
          <span className={s.emptyIcon}>📊</span>
          <span className={s.emptyText}>No benchmark sessions yet. Hit Record while in-game.</span>
        </div>
      )}

      {sessions.length > 0 && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          {[...sessions].reverse().map(session => (
            <button
              key={session.id}
              className={s.card}
              style={{ textAlign: 'left', cursor: 'pointer', border: '1px solid var(--color-border)', background: 'var(--color-bg-surface)', borderRadius: 'var(--radius-md)', padding: 'var(--space-4)' }}
              onClick={() => navigate(`/benchmarks/${session.id}`)}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                  <span style={{ fontSize: 'var(--text-sm)', fontWeight: 600, color: 'var(--color-text-primary)' }}>
                    {session.game_name ?? 'Unknown game'}
                  </span>
                  <span style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)' }}>
                    {new Date(session.timestamp).toLocaleString()} · {formatDuration(session.duration_secs)}
                  </span>
                </div>
                <div style={{ display: 'flex', gap: 16, fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>
                  {session.after?.fps_avg && (
                    <span style={{ color: 'var(--color-accent)' }}>{Math.round(session.after.fps_avg)} avg</span>
                  )}
                  {session.after?.fps_1pct_low && (
                    <span style={{ color: 'var(--color-warning)' }}>{Math.round(session.after.fps_1pct_low)} 1%</span>
                  )}
                </div>
              </div>
            </button>
          ))}
        </div>
      )}
    </main>
  );
}
