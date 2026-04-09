import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTelemetryStore } from '../stores/telemetryStore';
import { useGamesStore } from '../stores/gamesStore';
import { usePresetsStore } from '../stores/presetsStore';
import { useAppStore } from '../stores/appStore';
import { FrametimeChart } from '../components/FrametimeChart';
import { api } from '../lib/api';
import s from './Dashboard.module.css';

export function Dashboard() {
  const navigate = useNavigate();
  const { snapshot, history, startPolling, stopPolling } = useTelemetryStore();
  const { games, fetch: fetchGames } = useGamesStore();
  const { presets, fetch: fetchPresets } = usePresetsStore();
  const { serviceOnline, checkService, systemInfo } = useAppStore();
  const [launchingService, setLaunchingService] = useState(false);

  useEffect(() => {
    startPolling();
    fetchGames();
    fetchPresets();
    return () => stopPolling();
  }, [startPolling, stopPolling, fetchGames, fetchPresets]);

  const handleLaunchService = async () => {
    if (launchingService) return;
    setLaunchingService(true);
    try {
      await api.launchService();
      // Poll for service to come online (up to 12 s)
      let tries = 0;
      const poll = setInterval(async () => {
        tries++;
        await checkService();
        if (useAppStore.getState().serviceOnline || tries >= 12) {
          clearInterval(poll);
          setLaunchingService(false);
        }
      }, 1000);
    } catch {
      setLaunchingService(false);
    }
  };

  const fps       = snapshot?.frames?.fps_avg ?? 0;
  const fps1Low   = snapshot?.frames?.fps_1_percent_low ?? 0;
  const ftAvg     = snapshot?.frames?.frametime_avg_ms ?? 0;
  const gpuUsage  = snapshot?.gpu?.gpu_usage_pct ?? 0;
  const gpuTemp   = snapshot?.gpu?.gpu_temp_c ?? 0;
  const cpuUsage  = snapshot?.cpu?.usage_pct ?? 0;
  const cpuTemp   = snapshot?.cpu?.temp_c ?? 0;

  const appliedPresets = presets.filter(p => p.is_applied);
  const activePreset   = appliedPresets[0] ?? null;
  const activeGame     = games.find(g => g.id === activePreset?.id) || games[0];

  const frametimeData = history
    .map((h, i) => ({ t: i * 1000, ft: h?.frames?.frametime_avg_ms ?? 0 }))
    .filter(d => d.ft > 0);

  const gpuName = systemInfo?.gpu?.name ?? null;
  const cpuName = systemInfo?.cpu_name ?? null;

  return (
    <main className={s.page}>
      <div className={s.mainPanel}>
        {/* Service offline banner */}
        {!serviceOnline && (
          <div className={s.offlineBanner}>
            <div className={s.offlineBannerLeft}>
              <div className={s.offlineBannerDot} />
              <div className={s.offlineBannerTextGroup}>
                <span className={s.offlineBannerText}>SERVICE OFFLINE</span>
                <span className={s.offlineBannerSub}>Telemetria e otimizações indisponíveis no momento.</span>
              </div>
            </div>
            <button
              className={s.offlineBannerBtn}
              onClick={handleLaunchService}
              disabled={launchingService}
            >
              {launchingService ? (
                <>
                  <span className={s.spinner} />
                  INICIANDO...
                </>
              ) : 'EXECUTAR SERVIÇO'}
            </button>
          </div>
        )}

        {/* Bento Grid layout */}
        <div className={s.bentoGrid}>
          {/* Hero Tile: FPS */}
          <div className={`${s.tile} ${s.heroTile}`}>
            <div className={s.heroHeader}>
              <span className={s.heroTitle}>Frame Rate</span>
              <div className={s.badgeReady}>LIVE TELEMETRY</div>
            </div>
            <div className={s.heroValue}>
              {fps > 0 ? fps.toFixed(0) : '—'}
              <span className={s.unit}>FPS</span>
            </div>
            <div className={s.hwSub} style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <div className={s.statusDot} style={{ background: fps > 60 ? 'var(--color-accent)' : '#ffcc00' }} />
              {fps1Low > 0 ? `1% LOW: ${fps1Low.toFixed(0)} FPS` : 'SYSTEM STANDBY'}
            </div>
          </div>

          {/* Hardware Stats Tile */}
          <div className={`${s.tile} ${s.hwTile}`}>
            <span className={s.heroTitle}>Hardware Monitor</span>
            <div className={s.hwList}>
              <div className={s.hwItem}>
                <div className={s.hwInfo}>
                  <span className={s.hwLabel}>GPU — {gpuName?.split(' ').slice(-1)[0] ?? 'Graphics'}</span>
                  <div className={s.barContainer}>
                    <div className={s.barFill} style={{ width: `${gpuUsage}%` }} />
                  </div>
                </div>
                <div className={s.hwStat}>
                  <span className={s.hwValue}>{gpuUsage.toFixed(0)}%</span>
                  <div className={s.hwSub}>{gpuTemp}°C</div>
                </div>
              </div>
              <div className={s.hwItem}>
                <div className={s.hwInfo}>
                  <span className={s.hwLabel}>CPU — {cpuName?.split(' ').slice(0, 2).join(' ') ?? 'Processor'}</span>
                  <div className={s.barContainer}>
                    <div className={s.barFill} style={{ width: `${cpuUsage}%` }} />
                  </div>
                </div>
                <div className={s.hwStat}>
                  <span className={s.hwValue}>{cpuUsage.toFixed(0)}%</span>
                  <div className={s.hwSub}>{cpuTemp}°C</div>
                </div>
              </div>
            </div>
          </div>

          {/* Performance Meta Tile */}
          <div className={s.tile}>
            <span className={s.heroTitle}>LATENCY</span>
            <div className={s.hwStat} style={{ textAlign: 'left', marginTop: 'auto' }}>
              <span className={s.hwValue}>{ftAvg > 0 ? ftAvg.toFixed(1) : '—'}</span>
              <span className={s.unit}>ms</span>
              <div className={s.hwSub}>FRAME DELIVERY TIME</div>
            </div>
          </div>

          {/* Preset Tile */}
          <div className={s.tile} style={{ background: 'rgba(0, 212, 192, 0.05)', borderColor: 'rgba(0, 212, 192, 0.2)' }}>
            <span className={s.heroTitle} style={{ color: 'var(--color-accent)' }}>PROFILE ATIVO</span>
            <div style={{ marginTop: 'auto' }}>
              <div className={s.hwName} style={{ fontSize: '15px', fontWeight: 700, letterSpacing: '0.05em', color: 'var(--color-accent)' }}>
                {activePreset?.name.toUpperCase() ?? 'NONE DETECTED'}
              </div>
              <div className={s.hwSub}>ENGINE OPTIMIZATION</div>
            </div>
          </div>

          {/* Stability Chart Tile */}
          <div className={`${s.tile} ${s.chartTile}`}>
            <div className={s.chartHeader}>
              <span className={s.heroTitle}>Stability Analysis</span>
              <div className={s.badgeReady}>PRECISION POLLING</div>
            </div>
            <div style={{ flex: 1, minHeight: '120px' }}>
              {frametimeData.length > 2 ? (
                <FrametimeChart
                  data={frametimeData}
                  avgFps={fps}
                  p1Low={fps1Low}
                  p01Low={snapshot?.frames?.fps_0_1_percent_low ?? 0}
                  height={120}
                />
              ) : (
                <div style={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--color-text-muted)', fontSize: '12px' }}>
                  Aguardando telemetria secundária...
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Right Panel: Actions & Optimization */}
      <div className={s.rightPanel}>
        <div className={s.sectionTitle}>SABLE CORE</div>
        
        <button className={s.optimizeBtn} onClick={() => navigate('/optimizer')}>
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
            <path d="m12 14 4-4 4 4"/><path d="M12 4v10"/><path d="M20 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-4"/><path d="m4 14 4-4 4 4"/>
          </svg>
          OTIMIZAÇÃO RÁPIDA
        </button>

        <div className={s.rightDivider} />

        <div className={s.sectionTitle}>ACTIVE INSTANCE</div>
        <div className={s.presetCard}>
          <div className={s.presetName}>{activeGame?.name ?? 'DESCONHECIDO'}</div>
          <div className={s.hwSub} style={{ marginTop: '4px', fontSize: '11px', lineHeight: '1.4' }}>
            Nível de risco: {activePreset?.risk ?? 'Mínimo'}.
            <br />
            Monitoramento de latência habilitado.
          </div>
        </div>

        <div style={{ marginTop: 'auto', display: 'flex', flexDirection: 'column', gap: '8px' }}>
          <button className={s.configBtn} onClick={() => navigate('/settings')}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>
            </svg>
            CONFIGURAÇÕES DO SISTEMA
          </button>
          <div className={s.hwSub} style={{ textAlign: 'center', opacity: 0.5 }}>Sable Pro Beta v0.1.0</div>
        </div>
      </div>
    </main>
  );
}

