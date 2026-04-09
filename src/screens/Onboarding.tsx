import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAppStore } from '../stores/appStore';
import s from './Onboarding.module.css';

const TOTAL_STEPS = 4;

const STEPS = [
  {
    title: 'Welcome to Sable',
    desc: 'Sable is a premium Windows gaming optimizer built for serious PC gamers. It gives you real GPU telemetry, honest system optimization, and a clean in-game overlay — without the bloat.',
    bullets: [
      'Real-time GPU, CPU, and frametime metrics',
      'One-click optimization presets with full rollback',
      'Ultra-light GDI overlay — anti-cheat safe',
    ],
  },
  {
    title: 'How optimization works',
    desc: 'All presets are annotated with risk levels and applied with a full rollback snapshot. Every change is reversible in one click. Sable never makes changes it cannot undo.',
    bullets: [
      'Low-risk presets only touch well-known Windows settings',
      'Medium-risk presets require elevated service to be running',
      'Full rollback available at any time from the Optimizer screen',
    ],
  },
  {
    title: 'Start the service (optional)',
    desc: "For GPU telemetry and preset application, Sable's background service must be running. You can start it now or do it later from System settings.",
    bullets: [
      'Service runs at ~15 MB RAM — no idle CPU cost',
      'Communicates over a local named pipe (localhost only)',
      'Can be stopped or disabled at any time from Settings',
    ],
  },
  {
    title: 'Privacy & data collection',
    desc: 'Sable collects hardware telemetry (GPU, CPU, frametime) locally on your device to power the optimizer and overlay. No data is sent to external servers without your explicit consent.',
    bullets: [
      'All telemetry stays on-device — never uploaded automatically',
      'No account required — no email, no tracking',
      'You can disable all metric collection in Settings at any time',
    ],
  },
];

export function Onboarding() {
  const [step, setStep] = useState(0);
  const [consentChecked, setConsentChecked] = useState(false);
  const navigate = useNavigate();
  const { setFirstRunComplete, acceptTelemetryConsent } = useAppStore();

  const finish = () => {
    if (step === TOTAL_STEPS - 1 && !consentChecked) return;
    acceptTelemetryConsent();
    setFirstRunComplete();
    navigate('/', { replace: true });
  };

  const skip = () => {
    setFirstRunComplete();
    navigate('/', { replace: true });
  };

  const current = STEPS[step];
  const isPrivacyStep = step === TOTAL_STEPS - 1;
  const isLast = isPrivacyStep;
  const canFinish = !isPrivacyStep || consentChecked;

  return (
    <div className={s.page}>
      <div className={s.card}>
        <div className={s.logo}>
          <span className={s.accent}>S</span>able
        </div>

        <div className={s.steps}>
          {Array.from({ length: TOTAL_STEPS }).map((_, i) => (
            <div
              key={i}
              className={`${s.step} ${i <= step ? s.done : ''} ${i === step ? s.current : ''}`}
            />
          ))}
        </div>

        <div className={s.content}>
          <h1 className={s.stepTitle}>{current.title}</h1>
          <p className={s.stepDesc}>{current.desc}</p>
          <ul className={s.checkList}>
            {current.bullets.map((b, i) => (
              <li key={i} className={s.checkItem}>
                <span className={s.checkIcon}>✓</span>
                {b}
              </li>
            ))}
          </ul>

          {isPrivacyStep && (
            <label className={s.consentBox}>
              <input
                type="checkbox"
                className={s.checkbox}
                checked={consentChecked}
                onChange={e => setConsentChecked(e.target.checked)}
              />
              <span className={s.consentText}>
                I understand and agree to Sable's{' '}
                <span className={s.consentLink}>local-only data collection policy</span>.
                I can change this at any time in Settings.
              </span>
            </label>
          )}
        </div>

        <div className={s.footer}>
          {!isLast && (
            <button className={s.btnSkip} onClick={skip}>Skip</button>
          )}
          <button
            className={s.btnNext}
            onClick={isLast ? finish : () => setStep(step + 1)}
            disabled={!canFinish}
          >
            {isLast ? 'Get started' : 'Next →'}
          </button>
        </div>
      </div>
    </div>
  );
}
