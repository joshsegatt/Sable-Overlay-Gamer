// stores/benchmarkStore.ts — Benchmark sessions state

import { create } from 'zustand';
import { api, BenchmarkSession, BottleneckDiagnosis } from '../lib/api';

interface BenchmarkStore {
  sessions: BenchmarkSession[];
  isLoading: boolean;
  activeGameId: string | null;
  isBenchmarking: boolean;
  report: BottleneckDiagnosis[];

  fetch: () => Promise<void>;
  startBenchmark: (gameId: string) => Promise<void>;
  stopBenchmark: () => Promise<void>;
  fetchReport: (sessionId: string) => Promise<void>;
}

export const useBenchmarkStore = create<BenchmarkStore>((set) => ({
  sessions: [],
  isLoading: false,
  activeGameId: null,
  isBenchmarking: false,
  report: [],

  fetch: async () => {
    set({ isLoading: true });
    try {
      const sessions = await api.getBenchmarkSessions();
      set({ sessions: Array.isArray(sessions) ? sessions : [], isLoading: false });
    } catch {
      set({ isLoading: false });
    }
  },

  startBenchmark: async (gameId) => {
    await api.startBenchmark(gameId);
    set({ activeGameId: gameId, isBenchmarking: true });
  },

  stopBenchmark: async () => {
    await api.stopBenchmark();
    set({ isBenchmarking: false, activeGameId: null });
  },

  fetchReport: async (sessionId) => {
    try {
      const report = await api.getBottleneckReport(sessionId);
      set({ report: Array.isArray(report) ? report : [] });
    } catch {
      set({ report: [] });
    }
  },
}));
