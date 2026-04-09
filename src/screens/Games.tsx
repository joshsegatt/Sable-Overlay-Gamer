import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useGamesStore } from '../stores/gamesStore';
import { GameTile } from '../components/GameTile';
import s from './shared.module.css';

export function Games() {
  const navigate = useNavigate();
  const { games, isLoading, fetch } = useGamesStore();
  const [query, setQuery]  = useState('');

  useEffect(() => { fetch(); }, [fetch]);

  const filtered = query
    ? games.filter(g => g.name.toLowerCase().includes(query.toLowerCase()))
    : games;

  return (
    <main className={s.page}>
      <div className={s.header}>
        <div className={s.headerLeft}>
          <h1 className={s.pageTitle}>Games</h1>
          <p className={s.pageSubtitle}>{games.length} game{games.length !== 1 ? 's' : ''} detected</p>
        </div>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <input
            type="search"
            placeholder="Search…"
            value={query}
            onChange={e => setQuery(e.target.value)}
            style={{
              padding: '7px 12px',
              background: 'var(--color-bg-elevated)',
              border: '1px solid var(--color-border)',
              borderRadius: 'var(--radius-sm)',
              color: 'var(--color-text-primary)',
              fontSize: 'var(--text-sm)',
              outline: 'none',
              width: 200,
            }}
          />
          <button className={s.btnPrimary} onClick={fetch} disabled={isLoading}>
            {isLoading ? '…' : 'Rescan'}
          </button>
        </div>
      </div>

      {isLoading && (
        <div className={s.emptyState}>
          <span className={s.emptyText}>Scanning for games…</span>
        </div>
      )}

      {!isLoading && filtered.length === 0 && (
        <div className={s.emptyState}>
          <span className={s.emptyIcon}>🎮</span>
          <span className={s.emptyText}>
            {query ? 'No games match your search.' : 'No games found. Ensure Steam, Epic, or GOG is installed.'}
          </span>
        </div>
      )}

      {!isLoading && filtered.length > 0 && (
        <div className={s.gridGames}>
          {filtered.map(g => (
            <GameTile key={g.id} game={g} onClick={() => navigate(`/games/${g.id}`)} />
          ))}
        </div>
      )}
    </main>
  );
}
