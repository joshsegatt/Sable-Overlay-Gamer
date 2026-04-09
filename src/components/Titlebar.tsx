import { useAppStore } from '../stores/appStore';
import s from './Titlebar.module.css';

// Tauri window API — only available inside the native shell
const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function getWin() {
  if (!isTauri) return null;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  return getCurrentWindow();
}

export function Titlebar() {
  const serviceOnline = useAppStore(st => st.serviceOnline);

  const minimize     = async () => { const w = await getWin(); w?.minimize(); };
  const toggleMax    = async () => { const w = await getWin(); w?.toggleMaximize(); };
  const close        = async () => { const w = await getWin(); w?.close(); };

  return (
    <div className={s.bar} data-tauri-drag-region>
      <div className={s.left}>
        <div className={s.logoLockup}>
          <span className={s.logoText}>SABLE</span>
          <span className={s.badge}>PRO</span>
        </div>
      </div>

      <div className={s.center}>
        <div className={`${s.indicator} ${serviceOnline ? s.online : s.offline}`}>
          <div className={s.dot} />
          <span className={s.statusText}>
            {serviceOnline ? 'SYSTEM OPERATIONAL' : 'SERVICE OFFLINE'}
          </span>
        </div>
      </div>

      <div className={s.right}>
        <div className={s.controls}>
          <button className={s.winBtn} onClick={minimize} aria-label="Minimize">
            <svg width="12" height="12" viewBox="0 0 12 12"><rect x="2" y="5.5" width="8" height="1" fill="currentColor"/></svg>
          </button>
          <button className={s.winBtn} onClick={toggleMax} aria-label="Maximize">
            <svg width="12" height="12" viewBox="0 0 12 12"><rect x="2" y="2" width="8" height="8" rx="1" stroke="currentColor" fill="none"/></svg>
          </button>
          <button className={`${s.winBtn} ${s.winBtnClose}`} onClick={close} aria-label="Close">
            <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2.5 2.5L9.5 9.5M9.5 2.5L2.5 9.5" stroke="currentColor" strokeWidth="1.2"/></svg>
          </button>
        </div>
      </div>
    </div>
  );
}
