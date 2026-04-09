// stores/gamesStore.ts — Detected games state

import { create } from 'zustand';
import { api, GameEntry } from '../lib/api';

interface GamesStore {
  games: GameEntry[];
  isLoading: boolean;
  fetch: () => Promise<void>;
}

export const useGamesStore = create<GamesStore>((set) => ({
  games: [],
  isLoading: false,

  fetch: async () => {
    set({ isLoading: true });
    try {
      const games = await api.getGames();
      set({ games: Array.isArray(games) ? games : [], isLoading: false });
    } catch {
      set({ isLoading: false });
    }
  },
}));
