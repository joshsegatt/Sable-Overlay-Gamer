// components/MetricCard.tsx — core metric display widget

import { ResponsiveContainer, LineChart, Line } from 'recharts';
import s from './MetricCard.module.css';

interface MetricCardProps {
  label: string;
  value: number | null | undefined;
  unit?: string;
  delta?: number | null;       // positive = better, negative = worse
  deltaUnit?: string;
  sparklineData?: number[];
  sparklineColor?: string;
  invertDelta?: boolean;       // for metrics where lower is better (frametime, temp)
}

export function MetricCard({
  label,
  value,
  unit,
  delta,
  deltaUnit = '',
  sparklineData,
  sparklineColor = 'var(--color-accent)',
  invertDelta = false,
}: MetricCardProps) {
  const formattedValue = value !== null && value !== undefined
    ? Number.isInteger(value) ? value.toFixed(0) : value.toFixed(1)
    : null;

  const renderDelta = () => {
    if (delta === null || delta === undefined) return null;
    const isPositive = invertDelta ? delta < 0 : delta > 0;
    const isNegative = invertDelta ? delta > 0 : delta < 0;
    const className = isPositive
      ? `${s.delta} ${s.deltaPositive}`
      : isNegative
      ? `${s.delta} ${s.deltaNegative}`
      : `${s.delta} ${s.deltaNeutral}`;
    const sign = delta > 0 ? '+' : '';
    return (
      <span className={className}>
        {sign}{delta.toFixed(1)}{deltaUnit}
      </span>
    );
  };

  const chartData = sparklineData?.map((v) => ({ v })) ?? [];

  return (
    <div className={s.card}>
      <span className={s.label}>{label}</span>
      <div className={s.valueRow}>
        {formattedValue !== null ? (
          <>
            <span className={s.value}>{formattedValue}</span>
            {unit && <span className={s.unit}>{unit}</span>}
          </>
        ) : (
          <span className={s.null}>—</span>
        )}
        {renderDelta()}
      </div>
      {sparklineData && sparklineData.length > 1 && (
        <div className={s.sparkline}>
          <ResponsiveContainer width="100%" height={28}>
            <LineChart data={chartData}>
              <Line
                type="monotone"
                dataKey="v"
                stroke={sparklineColor}
                strokeWidth={1.5}
                dot={false}
                isAnimationActive={false}
              />
            </LineChart>
          </ResponsiveContainer>
        </div>
      )}
    </div>
  );
}
