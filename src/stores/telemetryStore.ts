// stores/telemetryStore.ts — Live telemetry state with polling

import { create } from 'zustand';
import { api, TelemetrySnapshot, defaultTelemetry } from '../lib/api';

interface TelemetryStore {
  snapshot: TelemetrySnapshot;
  history: TelemetrySnapshot[];
  isPolling: boolean;
  serviceOnline: boolean;
  pollingInterval: number | null;

  startPolling: (intervalMs?: number) => void;
  stopPolling: () => void;
  fetch: () => Promise<void>;
}

export const useTelemetryStore = create<TelemetryStore>((set, get) => ({
  snapshot: defaultTelemetry(),
  history: [],
  isPolling: false,
  serviceOnline: false,
  pollingInterval: null,

  fetch: async () => {
    try {
      const snapshot = await api.getTelemetry();
      set(st => ({
        snapshot,
        serviceOnline: true,
        history: [...st.history.slice(-59), snapshot],
      }));
    } catch {
      set({ serviceOnline: false });
    }
  },

  startPolling: (intervalMs = 1000) => {
    const existing = get().pollingInterval;
    if (existing !== null) clearInterval(existing);

    // Immediate first fetch
    get().fetch();

    const id = window.setInterval(() => {
      get().fetch();
    }, intervalMs);

    set({ isPolling: true, pollingInterval: id });
  },

  stopPolling: () => {
    const id = get().pollingInterval;
    if (id !== null) clearInterval(id);
    set({ isPolling: false, pollingInterval: null });
  },
}));
