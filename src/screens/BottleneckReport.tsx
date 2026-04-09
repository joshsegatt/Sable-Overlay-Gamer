import { useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useBenchmarkStore } from '../stores/benchmarkStore';
import s from './shared.module.css';

const kindColors: Record<string, string> = {
  GpuBound:       'var(--color-accent)',
  CpuBound:       'var(--color-warning)',
  VramPressure:   'var(--color-danger)',
  ThermalThrottle:'var(--color-danger)',
  PowerLimit:     'var(--color-warning)',
  LowGpuAnomaly:  'var(--color-text-muted)',
  BackgroundDrain:'var(--color-text-muted)',
  None:           'var(--color-positive)',
};

export function BottleneckReport() {
  const { sessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  const { report, fetchReport } = useBenchmarkStore();

  useEffect(() => {
    if (sessionId) fetchReport(sessionId);
  }, [sessionId, fetchReport]);

  return (
    <main className={s.page}>
      <div className={s.header}>
        <div className={s.headerLeft}>
          <button
            style={{ background: 'none', border: 'none', color: 'var(--color-text-muted)', cursor: 'pointer', fontSize: 'var(--text-sm)', marginBottom: 4, padding: 0 }}
            onClick={() => navigate(-1)}
          >
            ← Back
          </button>
          <h1 className={s.pageTitle}>Bottleneck Report</h1>
          <p className={s.pageSubtitle}>Automated performance diagnosis</p>
        </div>
      </div>

      {report.length === 0 && (
        <div className={s.emptyState}>
          <span className={s.emptyIcon}>✅</span>
          <span className={s.emptyText}>No bottlenecks detected. System is performing optimally.</span>
        </div>
      )}

      <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
        {report.map((item, i) => (
          <div key={i} className={s.card} style={{ borderLeft: `3px solid ${kindColors[item.kind] ?? 'var(--color-border)'}` }}>
            <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 16 }}>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                <span style={{ fontSize: 'var(--text-base)', fontWeight: 600, color: 'var(--color-text-primary)' }}>
                  {item.title}
                </span>
                <span style={{ fontSize: 'var(--text-sm)', color: 'var(--color-text-secondary)' }}>
                  {item.cause}
                </span>
                <span style={{ fontSize: 'var(--text-sm)', color: 'var(--color-text-primary)', marginTop: 4 }}>
                  💡 {item.recommendation}
                </span>
              </div>
              <span style={{
                fontFamily: 'var(--font-mono)',
                fontSize: 'var(--text-xs)',
                color: kindColors[item.kind] ?? 'var(--color-text-muted)',
                fontWeight: 600,
                flexShrink: 0,
              }}>
                {Math.round(item.confidence * 100)}%
              </span>
            </div>
          </div>
        ))}
      </div>
    </main>
  );
}
