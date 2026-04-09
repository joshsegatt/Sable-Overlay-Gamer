// stores/appStore.ts — Global app state (service status, settings, navigation)

import { create } from 'zustand';
import { api, AppSettings, SystemInfo, defaultSettings } from '../lib/api';

interface AppStore {
  serviceOnline: boolean;
  settings: AppSettings;
  systemInfo: SystemInfo | null;
  isFirstRun: boolean;
  telemetryConsent: boolean;
  expertMode: boolean;

  checkService: () => Promise<void>;
  fetchSystemInfo: () => Promise<void>;
  loadSettings: () => Promise<void>;
  saveSettings: (settings: AppSettings) => Promise<void>;
  setExpertMode: (enabled: boolean) => void;
  setFirstRunComplete: () => void;
  acceptTelemetryConsent: () => void;
}

export const useAppStore = create<AppStore>((set, get) => ({
  serviceOnline: false,
  settings: defaultSettings(),
  systemInfo: null,
  isFirstRun: !localStorage.getItem('sable_onboarded'),
  telemetryConsent: !!localStorage.getItem('sable_consent'),
  expertMode: false,

  checkService: async () => {
    const online = await api.ping();
    set({ serviceOnline: online });
  },

  fetchSystemInfo: async () => {
    try {
      const info = await api.getSystemInfo();
      set({ systemInfo: info as SystemInfo });
    } catch {
      // Non-critical — service may not be running
    }
  },

  loadSettings: async () => {
    try {
      const settings = await api.getSettings();
      set({ settings: settings as AppSettings, expertMode: (settings as AppSettings).expert_mode });
    } catch {
      // Use defaults
    }
  },

  saveSettings: async (settings) => {
    await api.saveSettings(settings);
    set({ settings, expertMode: settings.expert_mode });
  },

  setExpertMode: (enabled) => {
    const settings = { ...get().settings, expert_mode: enabled };
    set({ expertMode: enabled, settings });
    api.saveSettings(settings).catch(() => {});
  },

  setFirstRunComplete: () => {
    localStorage.setItem('sable_onboarded', '1');
    set({ isFirstRun: false });
  },

  acceptTelemetryConsent: () => {
    localStorage.setItem('sable_consent', '1');
    set({ telemetryConsent: true });
  },
}));
