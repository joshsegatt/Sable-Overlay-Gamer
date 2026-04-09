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
    setLaunchingService(true);
    try {
      await api.launchService();
      // Poll for service to come online (up to 8 s)
      let tries = 0;
      const poll = setInterval(async () => {
        tries++;
        await checkService();
        if (useAppStore.getState().serviceOnline || tries >= 8) clearInterval(poll);
      }, 1000);
    } finally {
      setLaunchingService(false);
    }
  };

  const fps       = snapshot?.frames?.fps_avg ?? 0;
  const fps1Low   = snapshot?.frames?.fps_1_percent_low ?? 0;
  const ftAvg     = snapshot?.frames?.frametime_avg_ms ?? 0;
  const ftStd     = snapshot?.frames?.frametime_stddev_ms ?? 0;
  const gpuUsage  = snapshot?.gpu?.gpu_usage_pct ?? 0;
  const gpuTemp   = snapshot?.gpu?.gpu_temp_c ?? 0;
  const cpuUsage  = snapshot?.cpu?.usage_pct ?? 0;
  const cpuTemp   = snapshot?.cpu?.temp_c ?? 0;

  const appliedPresets = presets.filter(p => p.is_applied);
  const activePreset   = appliedPresets[0] ?? null;
  const activeGame     = games[0] ?? null;

  const frametimeData = history
    .map((h, i) => ({ t: i * 1000, ft: h?.frames?.frametime_avg_ms ?? 0 }))
    .filter(d => d.ft > 0);

  const gpuName = systemInfo?.gpu?.name ?? null;
  const cpuName = systemInfo?.cpu_name ?? null;

  return (
    <main className={s.page}>
      {/* ── Left / main panel ─────────────────────────────────────────────── */}
      <div className={s.mainPanel}>

        {/* Service offline banner */}
        {!serviceOnline && (
          <div className={s.offlineBanner}>
            <div className={s.offlineBannerLeft}>
              <span className={s.offlineBannerDot} />
              <span className={s.offlineBannerText}>Service offline</span>
              <span className={s.offlineBannerSub}>— metrics and presets unavailable until the service is running.</span>
            </div>
            <button
              className={s.offlineBannerBtn}
              onClick={handleLaunchService}
              disabled={launchingService}
            >
              {launchingService ? 'Starting…' : 'Launch Service'}
            </button>
          </div>
        )}

        {/* Active game header */}
        <div className={s.gameHeader}>
          <div className={s.gameHeaderLeft}>
            <div className={s.gameIcon}>🎮</div>
            <h2 className={s.gameName}>{activeGame?.name ?? 'No Game Active'}</h2>
          </div>
          <div className={s.gameHeaderBadges}>
            <span className={s.badgeReady}>READY</span>
            <span className={serviceOnline ? s.badgeActive : s.badgeInactive}>
              {serviceOnline ? 'ACTIVE' : 'OFFLINE'}
            </span>
          </div>
        </div>

        {/* Hero metrics row */}
        <div className={s.metricsRow}>
          {/* FPS — hero */}
          <div className={s.heroMetric}>
            <span className={s.heroValue}>{fps > 0 ? fps.toFixed(0) : '—'}</span>
            <span className={s.heroLabel}>FPS</span>
            {fps1Low > 0 && (
              <span className={s.heroSub}>1% LOW: {fps1Low.toFixed(0)} FPS</span>
            )}
          </div>
          <div className={s.metricDivider} />

          {/* Frametime */}
          <div className={s.subMetric}>
            <span className={s.subValue}>{ftAvg > 0 ? `${ftAvg.toFixed(1)}ms` : '—'}</span>
            <span className={s.subLabel}>Frametime</span>
            {ftStd > 0 && <span className={s.subSub}>STD DEV: {ftStd.toFixed(1)}ms</span>}
          </div>
          <div className={s.metricDivider} />

          {/* GPU */}
          <div className={s.subMetric}>
            <span className={s.subValue}>{gpuUsage > 0 ? `${gpuUsage.toFixed(0)}%` : '—'}</span>
            <span className={s.subLabel}>GPU Usage</span>
            <span className={s.subSub}>
              {gpuName ?? '—'}{gpuTemp > 0 ? `  ${gpuTemp}°C` : ''}
            </span>
          </div>
          <div className={s.metricDivider} />

          {/* CPU */}
          <div className={s.subMetric}>
            <span className={s.subValue}>{cpuUsage > 0 ? `${cpuUsage.toFixed(0)}%` : '—'}</span>
            <span className={s.subLabel}>CPU Usage</span>
            <span className={s.subSub}>
              {cpuName ?? '—'}{cpuTemp > 0 ? `  ${cpuTemp}°C` : ''}
            </span>
          </div>
        </div>

        {/* Frametime stability chart */}
        <div className={s.chartSection}>
          <div className={s.chartHeader}>
            <span className={s.chartTitle}>
              Frametime Stability&nbsp;
              <span className={s.chartSubtitle}>(Last 5 Mins)</span>
            </span>
            <div className={s.chartBadges}>
              <span className={s.badgeReady}>READY</span>
              <span className={serviceOnline ? s.badgeActive : s.badgeInactive}>
                {serviceOnline ? 'ACTIVE' : 'OFFLINE'}
              </span>
            </div>
          </div>
          {frametimeData.length > 1 ? (
            <FrametimeChart
              data={frametimeData}
              avgFps={fps}
              p1Low={fps1Low}
              p01Low={snapshot?.frames?.fps_0_1_percent_low ?? 0}
              height={160}
            />
          ) : (
            <div className={s.chartEmpty}>
              No data yet — start a game session to begin recording.
            </div>
          )}
        </div>

        {/* Game profile section */}
        {activeGame && (
          <div className={s.gameProfileSection}>
            <span className={s.sectionTitle}>
              Game Profile: {activeGame.name}
            </span>
            <div className={s.profileCards}>
              <div className={s.profileCard}>
                <span className={s.profileCardLabel}>Auto-Optimize</span>
                <div className={s.toggleRow}>
                  <span className={s.toggleOn}>ON</span>
                  <span className={s.toggleDot} />
                </div>
                <button
                  className={s.historyBtn}
                  onClick={() => navigate(`/games/${activeGame.id}`)}
                >
                  Performance History →
                </button>
              </div>
              <div className={s.profileCard}>
                <span className={s.profileCardLabel}>Minimal Overlay</span>
                <div className={s.toggleRow}>
                  <span className={s.toggleOn}>ON</span>
                  <span className={s.toggleDot} />
                </div>
                <button
                  className={s.historyBtn}
                  onClick={() => navigate('/overlay')}
                >
                  Overlay Config →
                </button>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* ── Right panel ───────────────────────────────────────────────────── */}
      <div className={s.rightPanel}>
        <span className={s.rightPanelTitle}>Active Optimization</span>

        {activePreset ? (
          <div className={s.profileInfo}>
            <span className={s.profileInfoLabel}>Profile:</span>
            <div className={s.profileNameRow}>
              <span className={s.profileName}>{activePreset.name.toUpperCase()}</span>
              <span className={s.badgeActive}>ACTIVE</span>
            </div>
            <div className={s.profileStats}>
              <div className={s.profileStat}>
                <span className={s.profileStatLabel}>Latency:</span>
                <span className={s.profileStatValue}>Low</span>
              </div>
              <div className={s.profileStat}>
                <span className={s.profileStatLabel}>System Load:</span>
                <span className={s.profileStatValue}>Minimal</span>
              </div>
            </div>
          </div>
        ) : (
          <div className={s.noProfile}>
            <span className={s.noProfileText}>No active profile</span>
          </div>
        )}

        <div className={s.rightDivider} />

        <span className={s.rightPanelTitle}>Performance Comparison</span>
        <span className={s.compSubtitle}>Before vs After</span>

        <div className={s.compGrid}>
          <div className={s.compCard}>
            <span className={s.compLabel}>Before</span>
            <span className={s.compSublabel}>(Stock)</span>
            <div className={s.compStats}>
              <span className={s.compFpsLabel}>FPS:</span>
              <span className={s.compFpsValue}>—</span>
              <span className={s.compStatLabel}>1% Low:</span>
              <span className={s.compStatValue}>—</span>
              <span className={s.compStatLabel}>FT:</span>
              <span className={s.compStatValue}>—</span>
            </div>
          </div>

          <div className={`${s.compCard} ${s.compCardAfter}`}>
            <span className={s.compLabel}>After</span>
            <span className={s.compSublabel}>(Sable)</span>
            <div className={s.compStats}>
              <span className={s.compFpsLabel}>FPS:</span>
              <span className={`${s.compFpsValue} ${s.compAccent}`}>
                {fps > 0 ? fps.toFixed(0) : '—'}
              </span>
              <span className={s.compStatLabel}>1% Low:</span>
              <span className={s.compStatValue}>
                {fps1Low > 0 ? fps1Low.toFixed(0) : '—'}
              </span>
              <span className={s.compStatLabel}>FT:</span>
              <span className={s.compStatValue}>
                {ftAvg > 0 ? `${ftAvg.toFixed(1)}ms` : '—'}
              </span>
            </div>
          </div>
        </div>

        <div className={s.rightDivider} />

        <button className={s.optimizeBtn} onClick={() => navigate('/optimizer')}>
          🚀 Optimize System
        </button>
        <div className={s.actionRow}>
          <button className={s.actionBtn} onClick={() => navigate('/benchmarks')}>
            Save Profile
          </button>
          <button className={s.actionBtn} onClick={() => navigate('/optimizer')}>
            Advanced
          </button>
        </div>
      </div>
    </main>
  );
}

