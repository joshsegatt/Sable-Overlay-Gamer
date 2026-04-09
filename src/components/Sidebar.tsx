import { NavLink } from 'react-router-dom';
import { useAppStore } from '../stores/appStore';
import s from './Sidebar.module.css';

// ─── SVG icon components ─────────────────────────────────────────────────────

const IcoDashboard = () => (
  <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
    <rect x="1" y="1" width="6.5" height="6.5" rx="1.5" stroke="currentColor" strokeWidth="1.4"/>
    <rect x="10.5" y="1" width="6.5" height="6.5" rx="1.5" stroke="currentColor" strokeWidth="1.4"/>
    <rect x="1" y="10.5" width="6.5" height="6.5" rx="1.5" stroke="currentColor" strokeWidth="1.4"/>
    <rect x="10.5" y="10.5" width="6.5" height="6.5" rx="1.5" stroke="currentColor" strokeWidth="1.4"/>
  </svg>
);

const IcoGames = () => (
  <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
    <rect x="1.5" y="5" width="15" height="9" rx="2" stroke="currentColor" strokeWidth="1.4"/>
    <path d="M6 9H9M7.5 7.5V10.5" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round"/>
    <circle cx="11.5" cy="9" r="0.8" fill="currentColor"/>
    <circle cx="13.5" cy="9" r="0.8" fill="currentColor"/>
  </svg>
);

const IcoOptimizer = () => (
  <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
    <path d="M9 2L10.5 6.5H15L11 9.5L12.5 14L9 11L5.5 14L7 9.5L3 6.5H7.5L9 2Z" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round"/>
  </svg>
);

const IcoBenchmarks = () => (
  <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
    <path d="M2 14L6 9L9 11L12 6L16 10" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"/>
    <path d="M2 16H16" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round"/>
  </svg>
);

const IcoOverlay = () => (
  <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
    <rect x="2" y="3.5" width="14" height="10" rx="1.5" stroke="currentColor" strokeWidth="1.4"/>
    <path d="M2 14.5L16 14.5" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round"/>
    <path d="M7 16.5H11" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round"/>
  </svg>
);

const IcoSystem = () => (
  <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
    <rect x="1.5" y="1.5" width="15" height="11" rx="1.5" stroke="currentColor" strokeWidth="1.4"/>
    <path d="M5 15H13M9 12.5V15" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round"/>
    <path d="M5 7.5H13M5 5H8M5 10H10" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round"/>
  </svg>
);

const IcoSettings = () => (
  <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
    <circle cx="9" cy="9" r="2.5" stroke="currentColor" strokeWidth="1.4"/>
    <path d="M9 1.5V3M9 15V16.5M1.5 9H3M15 9H16.5M3.4 3.4L4.5 4.5M13.5 13.5L14.6 14.6M3.4 14.6L4.5 13.5M13.5 4.5L14.6 3.4" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round"/>
  </svg>
);

// ─── Nav definition ──────────────────────────────────────────────────────────

interface NavItem {
  to: string;
  icon: React.ReactNode;
  label: string;
}

const primary: NavItem[] = [
  { to: '/',          icon: <IcoDashboard />,  label: 'Dashboard' },
  { to: '/games',     icon: <IcoGames />,      label: 'Games' },
  { to: '/optimizer', icon: <IcoOptimizer />,  label: 'Optimizer' },
  { to: '/benchmarks',icon: <IcoBenchmarks />, label: 'Bench' },
];

const secondary: NavItem[] = [
  { to: '/overlay',   icon: <IcoOverlay />,    label: 'Overlay' },
  { to: '/system',    icon: <IcoSystem />,     label: 'System' },
  { to: '/settings',  icon: <IcoSettings />,   label: 'Settings' },
];

export function Sidebar() {
  const serviceOnline = useAppStore(s => s.serviceOnline);

  return (
    <nav className={s.sidebar} aria-label="Main navigation">
      <div className={s.section}>
        {primary.map(item => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === '/'}
            title={item.label}
            className={({ isActive }) => `${s.link} ${isActive ? s.active : ''}`}
          >
            <span className={s.icon}>{item.icon}</span>
            {item.label}
          </NavLink>
        ))}
      </div>

      <div className={s.divider} />

      <div className={s.section}>
        {secondary.map(item => (
          <NavLink
            key={item.to}
            to={item.to}
            title={item.label}
            className={({ isActive }) => `${s.link} ${isActive ? s.active : ''}`}
          >
            <span className={s.icon}>{item.icon}</span>
            {item.label}
          </NavLink>
        ))}
      </div>

      <div className={s.bottomSection}>
        <div className={`${s.statusDot} ${serviceOnline ? s.online : s.offline}`} />
        <span className={s.statusLabel}>{serviceOnline ? 'Online' : 'Offline'}</span>
      </div>
    </nav>
  );
}
