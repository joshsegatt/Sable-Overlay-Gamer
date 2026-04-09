import { useEffect, type ReactNode } from 'react';
import s from './DrawerPanel.module.css';

interface DrawerPanelProps {
  title: string;
  open: boolean;
  onClose: () => void;
  children: ReactNode;
}

export function DrawerPanel({ title, open, onClose, children }: DrawerPanelProps) {
  useEffect(() => {
    if (!open) return;
    const handle = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', handle);
    return () => window.removeEventListener('keydown', handle);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <>
      <div className={s.overlay} onClick={onClose} aria-hidden="true" />
      <aside className={s.drawer} role="dialog" aria-label={title}>
        <div className={s.header}>
          <span className={s.title}>{title}</span>
          <button className={s.closeBtn} onClick={onClose} aria-label="Close">✕</button>
        </div>
        <div className={s.body}>{children}</div>
      </aside>
    </>
  );
}
