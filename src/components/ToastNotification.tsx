import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react';
import s from './ToastNotification.module.css';

export type ToastVariant = 'success' | 'warning' | 'error' | 'info';

interface Toast {
  id: number;
  message: string;
  variant: ToastVariant;
}

interface ToastContext {
  toast: (message: string, variant?: ToastVariant) => void;
}

const Ctx = createContext<ToastContext>({ toast: () => {} });

const icons: Record<ToastVariant, string> = {
  success: '✓',
  warning: '⚠',
  error: '✕',
  info: 'ℹ',
};

let nextId = 0;
const AUTO_DISMISS_MS = 4000;
const MAX_TOASTS = 3;

function ToastItem({ toast, onDismiss }: { toast: Toast; onDismiss: () => void }) {
  useEffect(() => {
    const t = setTimeout(onDismiss, AUTO_DISMISS_MS);
    return () => clearTimeout(t);
  }, [onDismiss]);

  return (
    <div className={`${s.toast} ${s[toast.variant]}`} role="alert">
      <span className={s.icon} aria-hidden="true">{icons[toast.variant]}</span>
      <span className={s.message}>{toast.message}</span>
      <button className={s.dismiss} onClick={onDismiss} aria-label="Dismiss">✕</button>
    </div>
  );
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const dismiss = useCallback((id: number) => {
    setToasts(prev => prev.filter(t => t.id !== id));
  }, []);

  const toast = useCallback((message: string, variant: ToastVariant = 'info') => {
    const id = ++nextId;
    setToasts(prev => {
      const next = [...prev, { id, message, variant }];
      return next.length > MAX_TOASTS ? next.slice(next.length - MAX_TOASTS) : next;
    });
  }, []);

  return (
    <Ctx.Provider value={{ toast }}>
      {children}
      <div className={s.container}>
        {toasts.map(t => (
          <ToastItem key={t.id} toast={t} onDismiss={() => dismiss(t.id)} />
        ))}
      </div>
    </Ctx.Provider>
  );
}

export function useToast() {
  return useContext(Ctx);
}
