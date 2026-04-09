import {
  ResponsiveContainer,
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  ReferenceLine,
  CartesianGrid,
} from 'recharts';
import s from './FrametimeChart.module.css';

interface FrametimeEntry {
  t: number;   // timestamp ms offset
  ft: number;  // frametime ms
}

interface FrametimeChartProps {
  data: FrametimeEntry[];
  avgFps: number;
  p1Low: number;
  p01Low: number;
  height?: number;
}

export function FrametimeChart({ data, avgFps, p1Low, p01Low, height = 180 }: FrametimeChartProps) {
  const avgFt = avgFps > 0 ? 1000 / avgFps : 0;
  const p1Ft  = p1Low  > 0 ? 1000 / p1Low  : 0;
  const p01Ft = p01Low > 0 ? 1000 / p01Low : 0;

  return (
    <div className={s.wrapper}>
      <span className={s.title}>Frametime (ms)</span>
      <ResponsiveContainer width="100%" height={height}>
        <LineChart data={data} margin={{ top: 4, right: 8, left: -16, bottom: 0 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.04)" />
          <XAxis
            dataKey="t"
            tickFormatter={(v: number) => `${(v / 1000).toFixed(0)}s`}
            tick={{ fill: 'var(--color-text-muted)', fontSize: 10 }}
            axisLine={false}
            tickLine={false}
          />
          <YAxis
            tick={{ fill: 'var(--color-text-muted)', fontSize: 10, fontFamily: 'var(--font-mono)' }}
            axisLine={false}
            tickLine={false}
            unit="ms"
          />
          <Tooltip
            contentStyle={{
              background: 'var(--color-bg-elevated)',
              border: '1px solid var(--color-border)',
              borderRadius: 6,
              fontSize: 12,
              fontFamily: 'var(--font-mono)',
            }}
            labelStyle={{ color: 'var(--color-text-secondary)', fontSize: 11 }}
            itemStyle={{ color: 'var(--color-text-primary)' }}
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            formatter={((v: unknown) => [`${typeof v === 'number' ? v.toFixed(2) : '?'} ms`, 'Frametime']) as any}
          />
          {avgFt > 0 && (
            <ReferenceLine y={avgFt} stroke="var(--color-accent)" strokeDasharray="4 3" strokeOpacity={0.6}
              label={{ value: 'avg', fill: 'var(--color-accent)', fontSize: 9, position: 'right' }} />
          )}
          {p1Ft > 0 && (
            <ReferenceLine y={p1Ft} stroke="var(--color-warning)" strokeDasharray="4 3" strokeOpacity={0.5}
              label={{ value: '1%', fill: 'var(--color-warning)', fontSize: 9, position: 'right' }} />
          )}
          {p01Ft > 0 && (
            <ReferenceLine y={p01Ft} stroke="var(--color-danger)" strokeDasharray="4 3" strokeOpacity={0.5}
              label={{ value: '.1%', fill: 'var(--color-danger)', fontSize: 9, position: 'right' }} />
          )}
          <Line
            type="monotone"
            dataKey="ft"
            stroke="var(--color-accent)"
            strokeWidth={1.5}
            dot={false}
            isAnimationActive={false}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
