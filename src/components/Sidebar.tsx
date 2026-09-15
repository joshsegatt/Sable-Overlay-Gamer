import type { ReactNode } from 'react';
import { NavLink } from 'react-router-dom';
import s from './Sidebar.module.css';

const IcoDash = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
    <rect x="3" y="3" width="7" height="7" rx="1" />
    <rect x="14" y="3" width="7" height="7" rx="1" />
    <rect x="14" y="14" width="7" height="7" rx="1" />
    <rect x="3" y="14" width="7" height="7" rx="1" />
  </svg>
);
const IcoGames = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
    <rect x="2" y="7" width="20" height="10" rx="2" />
    <path d="M8 12h2M9 11v2" />
    <circle cx="16" cy="11" r="0.8" fill="currentColor" />
    <circle cx="18" cy="13" r="0.8" fill="currentColor" />
  </svg>
);
const IcoOverlay = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
    <rect x="3" y="4" width="18" height="14" rx="2" />
    <rect x="5" y="6" width="6" height="5" rx="1" />
  </svg>
);
const IcoPerf = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
    <path d="M3 12h4l3-8 4 16 3-8h4" />
  </svg>
);
const IcoProfiles = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
    <rect x="4" y="4" width="16" height="16" rx="2" />
    <path d="M4 10h16M10 20V10" />
  </svg>
);
const IcoSys = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
    <rect x="4" y="5" width="16" height="12" rx="2" />
    <path d="M8 21h8" />
  </svg>
);
const IcoGear = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
    <circle cx="12" cy="12" r="3" />
    <path d="M12 2v3M12 19v3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M2 12h3M19 12h3M4.9 19.1 7 17M17 7l2.1-2.1" />
  </svg>
);

const primary = [
  { to: '/', icon: <IcoDash />, label: 'Home', end: true },
  { to: '/games', icon: <IcoGames />, label: 'Games' },
  { to: '/overlay', icon: <IcoOverlay />, label: 'HUD' },
  { to: '/benchmarks', icon: <IcoPerf />, label: 'Bench' },
  { to: '/optimizer', icon: <IcoProfiles />, label: 'Tune' },
];

const secondary = [
  { to: '/system', icon: <IcoSys />, label: 'System' },
  { to: '/settings', icon: <IcoGear />, label: 'Settings' },
];

function Item({ to, icon, label, end }: { to: string; icon: ReactNode; label: string; end?: boolean }) {
  return (
    <NavLink
      to={to}
      end={end}
      title={label}
      className={({ isActive }) => `${s.link} ${isActive ? s.active : ''}`}
    >
      <span className={s.icon}>{icon}</span>
      <span className={s.label}>{label}</span>
    </NavLink>
  );
}

export function Sidebar() {
  return (
    <nav className={s.sidebar} aria-label="Main">
      <div className={s.nav}>
        {primary.map(item => <Item key={item.to} {...item} />)}
      </div>
      <div className={s.footer}>
        {secondary.map(item => <Item key={item.to} {...item} />)}
      </div>
    </nav>
  );
}
