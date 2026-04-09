import s from './MetricRow.module.css';

type Trend = 'up' | 'down' | 'flat';

interface MetricRowProps {
  label: string;
  value: string | number;
  trend?: Trend;
}

const trendIcon: Record<Trend, string> = { up: '↑', down: '↓', flat: '—' };

export function MetricRow({ label, value, trend }: MetricRowProps) {
  return (
    <div className={s.row}>
      <span className={s.label}>{label}</span>
      <div className={s.right}>
        <span className={s.value}>{value}</span>
        {trend && (
          <span className={`${s.trend} ${s[trend]}`} aria-hidden="true">
            {trendIcon[trend]}
          </span>
        )}
      </div>
    </div>
  );
}
