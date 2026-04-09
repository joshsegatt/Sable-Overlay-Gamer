import { useEffect } from 'react';
import s from './ConfirmModal.module.css';

interface ConfirmModalProps {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  destructive?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmModal({
  open,
  title,
  message,
  confirmLabel = 'Confirm',
  destructive = false,
  onConfirm,
  onCancel,
}: ConfirmModalProps) {
  useEffect(() => {
    if (!open) return;
    const handle = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onCancel();
      if (e.key === 'Enter') onConfirm();
    };
    window.addEventListener('keydown', handle);
    return () => window.removeEventListener('keydown', handle);
  }, [open, onCancel, onConfirm]);

  if (!open) return null;

  return (
    <div className={s.backdrop} onClick={onCancel}>
      <div
        className={s.modal}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        onClick={(e) => e.stopPropagation()}
      >
        <p id="confirm-title" className={s.title}>{title}</p>
        <p className={s.body}>{message}</p>
        <div className={s.footer}>
          <button className={`${s.btn} ${s.btnCancel}`} onClick={onCancel}>Cancel</button>
          <button
            className={`${s.btn} ${destructive ? s.btnConfirm : s.btnConfirmSafe}`}
            onClick={onConfirm}
            autoFocus
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
