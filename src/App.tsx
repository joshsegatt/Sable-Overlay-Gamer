import { Component, lazy, Suspense, useEffect, type ReactNode } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { useAppStore } from './stores/appStore';
import { Titlebar } from './components/Titlebar';
import { Sidebar } from './components/Sidebar';
import { ToastProvider } from './components/ToastNotification';

// ─── Error Boundary ───────────────────────────────────────────────────────────

interface EBState { error: Error | null; }

class ErrorBoundary extends Component<{ children: ReactNode }, EBState> {
  state: EBState = { error: null };

  static getDerivedStateFromError(error: Error): EBState {
    return { error };
  }

  render() {
    if (this.state.error) {
      return (
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 16, padding: 40, color: 'var(--color-text-primary)' }}>
          <span style={{ fontSize: 32 }}>⚠</span>
          <p style={{ fontWeight: 600, margin: 0 }}>Something went wrong</p>
          <p style={{ fontSize: 'var(--text-sm)', color: 'var(--color-text-muted)', margin: 0, maxWidth: 400, textAlign: 'center' }}>
            {this.state.error.message}
          </p>
          <button
            onClick={() => this.setState({ error: null })}
            style={{ background: 'var(--color-accent)', color: '#fff', border: 'none', borderRadius: 'var(--radius-md)', padding: '8px 20px', cursor: 'pointer', fontWeight: 600 }}
          >
            Try again
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

// Screens — lazy loaded
const Onboarding     = lazy(() => import('./screens/Onboarding').then(m => ({ default: m.Onboarding })));
const Dashboard      = lazy(() => import('./screens/Dashboard').then(m => ({ default: m.Dashboard })));
const Games          = lazy(() => import('./screens/Games').then(m => ({ default: m.Games })));
const GameProfile    = lazy(() => import('./screens/GameProfile').then(m => ({ default: m.GameProfile })));
const Optimizer      = lazy(() => import('./screens/Optimizer').then(m => ({ default: m.Optimizer })));
const Benchmarks     = lazy(() => import('./screens/Benchmarks').then(m => ({ default: m.Benchmarks })));
const BenchmarkDetail= lazy(() => import('./screens/BenchmarkDetail').then(m => ({ default: m.BenchmarkDetail })));
const OverlayConfig  = lazy(() => import('./screens/OverlayConfig').then(m => ({ default: m.OverlayConfig })));
const SystemInfo     = lazy(() => import('./screens/SystemInfo').then(m => ({ default: m.SystemInfo })));
const Settings       = lazy(() => import('./screens/Settings').then(m => ({ default: m.Settings })));
const BottleneckReport = lazy(() => import('./screens/BottleneckReport').then(m => ({ default: m.BottleneckReport })));

const Spinner = () => (
  <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--color-text-muted)' }}>
    …
  </div>
);

function AppLayout() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden' }}>
      <Titlebar />
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        <Sidebar />
        <div style={{ flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
          <ErrorBoundary>
            <Suspense fallback={<Spinner />}>
              <Routes>
              <Route path="/" element={<Dashboard />} />
              <Route path="/games" element={<Games />} />
              <Route path="/games/:id" element={<GameProfile />} />
              <Route path="/optimizer" element={<Optimizer />} />
              <Route path="/benchmarks" element={<Benchmarks />} />
              <Route path="/benchmarks/:id" element={<BenchmarkDetail />} />
              <Route path="/overlay" element={<OverlayConfig />} />
              <Route path="/system" element={<SystemInfo />} />
              <Route path="/settings" element={<Settings />} />
              <Route path="/report/:sessionId" element={<BottleneckReport />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
            </Suspense>
          </ErrorBoundary>
        </div>
      </div>
    </div>
  );
}

export function App() {
  const { isFirstRun, checkService, loadSettings, fetchSystemInfo } = useAppStore();

  useEffect(() => {
    checkService();
    loadSettings();
    fetchSystemInfo();
  }, [checkService, loadSettings, fetchSystemInfo]);

  return (
    <ToastProvider>
      <BrowserRouter>
        <Suspense fallback={<Spinner />}>
          {isFirstRun ? (
            <Routes>
              <Route path="*" element={<Onboarding />} />
            </Routes>
          ) : (
            <AppLayout />
          )}
        </Suspense>
      </BrowserRouter>
    </ToastProvider>
  );
}

export default App;
