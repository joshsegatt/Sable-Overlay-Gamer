import { useAppStore } from '../stores/appStore';
import s from './Titlebar.module.css';

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function getWin() {
  if (!isTauri) return null;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  return getCurrentWindow();
}

export function Titlebar() {
  const serviceOnline = useAppStore(st => st.serviceOnline);

  const minimize = async () => { (await getWin())?.minimize(); };
  const toggleMax = async () => { (await getWin())?.toggleMaximize(); };
  const close = async () => { (await getWin())?.close(); };

  return (
    <div className={s.bar} data-tauri-drag-region>
      <div className={s.left} data-tauri-drag-region>
        <span className={s.mark} aria-hidden />
        <span className={s.logoText}>SABLE</span>
      </div>

      <div className={s.center} data-tauri-drag-region>
        <div className={`${s.indicator} ${serviceOnline ? s.online : ''}`}>
          <span className={s.dot} />
          <span className={s.statusText}>
            {serviceOnline ? 'Service running' : 'Service offline'}
          </span>
        </div>
      </div>

      <div className={s.right}>
        <div className={s.controls}>
          <button className={s.winBtn} onClick={minimize} aria-label="Minimize">
            <svg width="10" height="10" viewBox="0 0 12 12"><rect x="2" y="5.5" width="8" height="1" fill="currentColor"/></svg>
          </button>
          <button className={s.winBtn} onClick={toggleMax} aria-label="Maximize">
            <svg width="10" height="10" viewBox="0 0 12 12"><rect x="2.5" y="2.5" width="7" height="7" stroke="currentColor" fill="none"/></svg>
          </button>
          <button className={`${s.winBtn} ${s.winBtnClose}`} onClick={close} aria-label="Close">
            <svg width="10" height="10" viewBox="0 0 12 12"><path d="M3 3L9 9M9 3L3 9" stroke="currentColor" strokeWidth="1.2"/></svg>
          </button>
        </div>
      </div>
    </div>
  );
}
