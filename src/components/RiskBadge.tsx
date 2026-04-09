import type { RiskLevel } from '../lib/api';
import s from './RiskBadge.module.css';

interface RiskBadgeProps {
  risk: RiskLevel;
}

const icons: Record<RiskLevel, string> = {
  Low: '●',
  Medium: '▲',
  High: '■',
};

const labels: Record<RiskLevel, string> = {
  Low: 'Low risk',
  Medium: 'Medium risk',
  High: 'High risk',
};

export function RiskBadge({ risk }: RiskBadgeProps) {
  return (
    <span className={`${s.badge} ${s[risk.toLowerCase() as 'low' | 'medium' | 'high']}`}>
      <span aria-hidden="true">{icons[risk]}</span>
      {labels[risk]}
    </span>
  );
}
