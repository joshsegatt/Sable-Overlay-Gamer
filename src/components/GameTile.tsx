import type { GameEntry } from '../lib/api';
import s from './GameTile.module.css';

interface GameTileProps {
  game: GameEntry;
  activePresetName?: string;
  onClick?: () => void;
}

const platformIcons: Record<string, string> = {
  Steam: '🎮',
  Epic: '🟣',
  Gog: '⚪',
  Ea: '🟠',
  Manual: '📁',
  Unknown: '❓',
};

export function GameTile({ game, activePresetName, onClick }: GameTileProps) {
  const icon = platformIcons[game.platform] ?? '🎮';

  return (
    <article
      className={s.tile}
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && onClick?.()}
      aria-label={game.name}
    >
      {game.icon_path ? (
        <img className={s.cover} src={game.icon_path} alt={game.name} loading="lazy" />
      ) : (
        <div className={s.coverPlaceholder}>
          <span aria-hidden="true">🎮</span>
        </div>
      )}

      <div className={s.body}>
        <p className={s.name} title={game.name}>{game.name}</p>
        <div className={s.meta}>
          <span className={s.platform}>{icon} {game.platform}</span>
          {activePresetName && (
            <span className={s.activePreset} title={activePresetName}>{activePresetName}</span>
          )}
        </div>
      </div>
    </article>
  );
}
