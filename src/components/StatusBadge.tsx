// components/StatusBadge.tsx

import s from './StatusBadge.module.css';

type Status = 'applied' | 'not-applied' | 'modified' | 'online' | 'offline';

const labels: Record<Status, string> = {
  'applied':     'Applied',
  'not-applied': 'Not Applied',
  'modified':    'Modified',
  'online':      'Online',
  'offline':     'Offline',
};

const styles: Record<Status, string> = {
  'applied':     s.applied,
  'not-applied': s.notApplied,
  'modified':    s.modified,
  'online':      s.online,
  'offline':     s.offline,
};

interface StatusBadgeProps {
  status: Status;
  label?: string;
}

export function StatusBadge({ status, label }: StatusBadgeProps) {
  return (
    <span className={`${s.badge} ${styles[status]}`}>
      <span className={s.dot} />
      {label ?? labels[status]}
    </span>
  );
}
