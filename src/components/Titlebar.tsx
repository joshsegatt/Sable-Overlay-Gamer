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
  const systemInfo = useAppStore(st => st.systemInfo);

  const minimize     = async () => { const w = await getWin(); w?.minimize(); };
  const toggleMax    = async () => { const w = await getWin(); w?.toggleMaximize(); };
  const close        = async () => { const w = await getWin(); w?.close(); };

  return (
    <div className={s.bar}>
      <div className={s.left}>
        <span className={s.logo}>
          <span className={s.accent}>S</span>able
        </span>
      </div>

      <div className={s.center}>
        <span className={serviceOnline ? s.statusOnline : s.statusOffline}>
          {serviceOnline ? '✓ SYSTEM OPTIMIZED' : '○ SERVICE OFFLINE'}
        </span>
        {systemInfo?.gpu && (
          <span className={s.apiTag}>API: DIRECTX 12</span>
        )}
      </div>

      <div className={s.right}>
        <span className={s.userSection}>
          <span className={s.userAvatar}>👤</span>
          <span className={s.userName}>User</span>
        </span>
        <div className={s.controls}>
          <button className={s.winBtn} onClick={minimize} aria-label="Minimize" title="Minimize">─</button>
          <button className={s.winBtn} onClick={toggleMax} aria-label="Maximize" title="Maximize">□</button>
          <button className={`${s.winBtn} ${s.winBtnClose}`} onClick={close} aria-label="Close" title="Close">✕</button>
        </div>
      </div>
    </div>
  );
}
