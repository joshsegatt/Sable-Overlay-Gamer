import { useParams, useNavigate } from 'react-router-dom';
import { lazy, Suspense } from 'react';
import { useBenchmarkStore } from '../stores/benchmarkStore';
import { usePresetsStore } from '../stores/presetsStore';
import s from './shared.module.css';

const FrametimeChart = lazy(() =>
  import('../components/FrametimeChart').then(m => ({ default: m.FrametimeChart }))
);

function MetricBar({ label, before, after, unit, higherIsBetter = true }: {
  label: string; before: number; after: number; unit: string; higherIsBetter?: boolean;
}) {
  const delta = after - before;
  const improved = higherIsBetter ? delta > 0 : delta < 0;
  const neutral = Math.abs(delta) < 0.05;
  const pct = before !== 0 ? Math.abs(delta / before) * 100 : 0;
  const accentColor = neutral
    ? 'var(--color-text-muted)'
    : improved ? 'var(--color-positive)' : 'var(--color-danger)';
  const sign = delta > 0 ? '+' : '';

  const maxVal = Math.max(before, after) || 1;
  const beforeWidth = (before / maxVal) * 100;
  const afterWidth = (after / maxVal) * 100;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline' }}>
        <span style={{ fontSize: 'var(--text-xs)', fontWeight: 500, color: 'var(--color-text-secondary)', textTransform: 'uppercase', letterSpacing: 'var(--tracking-wider)' }}>{label}</span>
        <span style={{ fontSize: 'var(--text-sm)', fontFamily: 'var(--font-mono)', fontWeight: 600, color: accentColor }}>
          {!neutral && <>{sign}{delta.toFixed(1)}{unit} </>}
          <span style={{ fontSize: 'var(--text-xs)', opacity: 0.75 }}>
            {neutral ? 'no change' : `(${sign}${pct.toFixed(1)}%)`}
          </span>
        </span>
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 5 }}>
        {/* Before bar */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <span style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', width: 40, flexShrink: 0, textAlign: 'right' }}>Before</span>
          <div style={{ flex: 1, height: 6, background: 'var(--color-bg-base)', borderRadius: 3, overflow: 'hidden' }}>
            <div style={{ height: '100%', width: `${beforeWidth}%`, background: 'var(--color-border-strong)', borderRadius: 3, transition: 'width 400ms var(--ease-out)' }} />
          </div>
          <span style={{ fontSize: 'var(--text-xs)', fontFamily: 'var(--font-mono)', color: 'var(--color-text-muted)', width: 52, flexShrink: 0 }}>{before.toFixed(1)}{unit}</span>
        </div>
        {/* After bar */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <span style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-primary)', width: 40, flexShrink: 0, textAlign: 'right', fontWeight: 500 }}>After</span>
          <div style={{ flex: 1, height: 6, background: 'var(--color-bg-base)', borderRadius: 3, overflow: 'hidden' }}>
            <div style={{ height: '100%', width: `${afterWidth}%`, background: accentColor, borderRadius: 3, boxShadow: neutral ? 'none' : `0 0 6px ${accentColor}60`, transition: 'width 400ms var(--ease-out)' }} />
          </div>
          <span style={{ fontSize: 'var(--text-xs)', fontFamily: 'var(--font-mono)', color: 'var(--color-text-primary)', fontWeight: 600, width: 52, flexShrink: 0 }}>{after.toFixed(1)}{unit}</span>
        </div>
      </div>
    </div>
  );
}

function DeltaCard({ label, delta, unit, higherIsBetter = true }: {
  label: string; delta: number; unit: string; higherIsBetter?: boolean;
}) {
  const improved = higherIsBetter ? delta > 0 : delta < 0;
  const neutral = Math.abs(delta) < 0.05;
  const color = neutral ? 'var(--color-text-muted)' : improved ? 'var(--color-positive)' : 'var(--color-danger)';
  const bg = neutral ? 'transparent' : improved ? 'var(--color-positive-dim)' : 'var(--color-danger-dim)';
  return (
    <div style={{ background: 'var(--color-bg-elevated)', border: `1px solid var(--color-border)`, borderRadius: 'var(--radius-md)', padding: '12px 16px', display: 'flex', flexDirection: 'column', gap: 4, boxShadow: 'var(--shadow-card)' }}>
      <span style={{ fontSize: 10, fontWeight: 600, color: 'var(--color-text-muted)', textTransform: 'uppercase', letterSpacing: '0.1em' }}>{label}</span>
      <span style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xl)', fontWeight: 700, color, padding: '2px 6px', borderRadius: 'var(--radius-sm)', background: bg, display: 'inline-block' }}>
        {delta > 0 ? '+' : ''}{delta.toFixed(1)}{unit}
      </span>
    </div>
  );
}

export function BenchmarkDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const sessions = useBenchmarkStore(st => st.sessions);
  const presets = usePresetsStore(st => st.presets);
  const session = sessions.find(s => s.id === id);

  if (!session) {
    return (
      <main className={s.page}>
        <div className={s.emptyState}>
          <span className={s.emptyIcon}>❓</span>
          <span className={s.emptyText}>Session not found.</span>
          <button className={s.btnGhost} onClick={() => navigate('/benchmarks')}>← Back</button>
        </div>
      </main>
    );
  }

  // Accumulate real elapsed time from actual frametimes instead of hardcoding 16ms intervals
  let accumMs = 0;
  const chartData = session.frametime_history.map((ft) => {
    const t = accumMs;
    accumMs += ft;
    return { t, ft };
  });

  // Look up preset name from store; fall back to showing "Custom" if not found
  const presetName = session.preset_applied
    ? presets.find(p => p.id === session.preset_applied)?.name ?? 'Custom'
    : null;

  return (
    <main className={s.page}>
      <div className={s.header}>
        <div className={s.headerLeft}>
          <button
            style={{ background: 'none', border: 'none', color: 'var(--color-text-muted)', cursor: 'pointer', fontSize: 'var(--text-sm)', marginBottom: 4, padding: 0 }}
            onClick={() => navigate('/benchmarks')}
          >
            ← Benchmarks
          </button>
          <h1 className={s.pageTitle}>{session.game_name}</h1>
          <p className={s.pageSubtitle}>
            {new Date(session.timestamp).toLocaleString()} · {session.duration_secs}s
            {presetName ? ` · Preset: ${presetName}` : ''}
          </p>
        </div>
      </div>

      {/* Frametime chart */}
      {chartData.length > 0 && (
        <div className={s.card}>
          <Suspense fallback={<div style={{ height: 180 }} />}>
            <FrametimeChart
              data={chartData}
              avgFps={session.after?.fps_avg ?? session.before?.fps_avg ?? 0}
              p1Low={session.after?.fps_1pct_low ?? session.before?.fps_1pct_low ?? 0}
              p01Low={0}
            />
          </Suspense>
        </div>
      )}

      {/* Before vs After comparison */}
      {session.before && session.after && (() => {
        const b = session.before!;
        const a = session.after!;
        return (
          <div className={s.section}>
            <span className={s.sectionTitle}>Before vs After</span>

            {/* Delta summary cards */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(130px, 1fr))', gap: 10, marginBottom: 16 }}>
              <DeltaCard label="Avg FPS" delta={a.fps_avg - b.fps_avg} unit="" higherIsBetter />
              <DeltaCard label="1% Low" delta={a.fps_1pct_low - b.fps_1pct_low} unit="" higherIsBetter />
              <DeltaCard label="Frametime" delta={a.frametime_avg_ms - b.frametime_avg_ms} unit="ms" higherIsBetter={false} />
              <DeltaCard label="GPU Temp" delta={a.gpu_temp_c - b.gpu_temp_c} unit="°C" higherIsBetter={false} />
            </div>

            {/* Detailed bars */}
            <div className={s.card} style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
              <MetricBar label="Avg FPS" before={b.fps_avg} after={a.fps_avg} unit="" />
              <MetricBar label="1% Low FPS" before={b.fps_1pct_low} after={a.fps_1pct_low} unit="" />
              <MetricBar label="Frametime avg" before={b.frametime_avg_ms} after={a.frametime_avg_ms} unit="ms" higherIsBetter={false} />
              <MetricBar label="Frametime p99" before={b.frametime_p99_ms} after={a.frametime_p99_ms} unit="ms" higherIsBetter={false} />
              <MetricBar label="GPU Usage" before={b.gpu_usage_pct} after={a.gpu_usage_pct} unit="%" />
              <MetricBar label="GPU Temp" before={b.gpu_temp_c} after={a.gpu_temp_c} unit="°C" higherIsBetter={false} />
            </div>
          </div>
        );
      })()}

      {/* Single pass summary */}
      {!session.before && session.after && (
        <div className={s.section}>
          <span className={s.sectionTitle}>Summary</span>
          <div className={s.card} style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            {[
              { label: 'Avg FPS', value: session.after.fps_avg.toFixed(0) },
              { label: '1% Low', value: session.after.fps_1pct_low.toFixed(0) },
              { label: 'Frametime avg', value: `${session.after.frametime_avg_ms.toFixed(2)} ms` },
              { label: 'GPU Usage', value: `${session.after.gpu_usage_pct.toFixed(0)}%` },
              { label: 'GPU Temp', value: `${session.after.gpu_temp_c.toFixed(0)}°C` },
              { label: 'CPU Usage', value: `${session.after.cpu_usage_pct.toFixed(0)}%` },
            ].map(({ label, value }) => (
              <div key={label} style={{ display: 'flex', justifyContent: 'space-between', padding: '8px 0', borderBottom: '1px solid var(--color-border)' }}>
                <span style={{ fontSize: 'var(--text-sm)', color: 'var(--color-text-secondary)' }}>{label}</span>
                <span style={{ fontSize: 'var(--text-sm)', fontFamily: 'var(--font-mono)', fontWeight: 600, color: 'var(--color-text-primary)' }}>{value}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </main>
  );
}
