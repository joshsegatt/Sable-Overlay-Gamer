// stores/presetsStore.ts — Optimization presets state

import { create } from 'zustand';
import { api, Preset } from '../lib/api';

interface PresetsStore {
  presets: Preset[];
  isLoading: boolean;
  applyingId: string | null;
  rollingBackId: string | null;

  fetch: () => Promise<void>;
  apply: (presetId: string) => Promise<void>;
  rollback: (presetId: string) => Promise<void>;
}

export const usePresetsStore = create<PresetsStore>((set) => ({
  presets: [],
  isLoading: false,
  applyingId: null,
  rollingBackId: null,

  fetch: async () => {
    set({ isLoading: true });
    try {
      const presets = await api.getPresets();
      set({ presets: Array.isArray(presets) ? presets : [], isLoading: false });
    } catch {
      set({ isLoading: false });
    }
  },

  apply: async (presetId) => {
    set({ applyingId: presetId });
    try {
      await api.applyPreset(presetId);
      // Update local state optimistically
      set((s) => ({
        presets: s.presets.map((p) =>
          p.id === presetId ? { ...p, is_applied: true } : p
        ),
        applyingId: null,
      }));
    } catch (e) {
      set({ applyingId: null });
      throw e;
    }
  },

  rollback: async (presetId) => {
    set({ rollingBackId: presetId });
    try {
      await api.rollbackPreset(presetId);
      set((s) => ({
        presets: s.presets.map((p) =>
          p.id === presetId ? { ...p, is_applied: false } : p
        ),
        rollingBackId: null,
      }));
    } catch (e) {
      set({ rollingBackId: null });
      throw e;
    }
  },
}));
