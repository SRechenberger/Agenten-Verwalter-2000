import { useEffect, useEffectEvent, useState } from 'react';
import type { CSSProperties, ReactElement, RefObject } from 'react';
import { downloadPercent } from '@/features/voice/downloadProgress';
import { useDictation } from '@/features/voice/useDictation';
import type { DictationControl } from '@/features/voice/useDictation';
import { VoiceSetup } from '@/features/voice/VoiceSetup';
import type { VoiceModelState } from '@/lib/bindings/VoiceModelState';
import { useVoiceStore } from '@/stores/voice';
import type { DictationPhase } from '@/stores/voice';
import './VoiceButton.css';

const BAR_NUMBERS = [1, 2, 3] as const;
const DOT_NUMBERS = [1, 2, 3] as const;

interface VoiceButtonProps {
  /** Schlüssel der Eingabe: Session-ID bzw. `NEW_SESSION_OWNER`. */
  owner: string;
  inputRef: RefObject<HTMLTextAreaElement | null>;
  getValue: () => string;
  setValue: (next: string) => void;
  onError: (message: string | null) => void;
  /** Wo das Menü „Diktieren einrichten“ aufgeht. */
  setupPlacement: 'above' | 'below';
  disabled: boolean;
}

/** Mikrofon-Knopf in der Textzeile einer Eingabe: startet und beendet das Diktat, zeigt Pegel und Erkennung und öffnet
 *  bei fehlendem Sprachmodell das Einrichten. `Strg+M` im Textfeld schaltet dasselbe um. */
export function VoiceButton({
  owner,
  inputRef,
  getValue,
  setValue,
  onError,
  setupPlacement,
  disabled,
}: VoiceButtonProps): ReactElement {
  const [isSetupOpen, setIsSetupOpen] = useState<boolean>(false);
  const modelState: VoiceModelState | null = useVoiceStore((state) => state.modelState);
  const dictation: DictationControl = useDictation({
    owner,
    inputRef,
    getValue,
    setValue,
    onError,
    onNeedsSetup: openSetup,
  });
  const { phase, isBlocked } = dictation;
  const isActive: boolean = phase !== 'idle';
  const isButtonDisabled: boolean = disabled || isBlocked;
  const needsSetup: boolean = modelState !== null && modelState.kind !== 'ready';
  const tooltip: string = tooltipFor(phase, isBlocked, modelState);

  const handleShortcut = useEffectEvent((event: KeyboardEvent): void => {
    const isMicrophoneKey: boolean =
      event.ctrlKey && !event.shiftKey && !event.altKey && event.key.toLowerCase() === 'm';
    if (!isMicrophoneKey) {
      return;
    }
    event.preventDefault();
    if (!isButtonDisabled) {
      activate();
    }
  });

  useEffect(() => {
    const element: HTMLTextAreaElement | null = inputRef.current;
    if (element === null) {
      return undefined;
    }
    function handleKeyDown(event: KeyboardEvent): void {
      handleShortcut(event);
    }
    element.addEventListener('keydown', handleKeyDown);
    return (): void => {
      element.removeEventListener('keydown', handleKeyDown);
    };
  }, [inputRef]);

  // Der Core sagt „fehlt“, obwohl der Store „fertig“ oder noch nichts wusste (Datei von Hand gelöscht).
  function openSetup(): void {
    const store = useVoiceStore.getState();
    if (store.modelState === null || store.modelState.kind === 'ready') {
      store.setModelState({ kind: 'missing' });
    }
    setIsSetupOpen(true);
  }

  function closeSetup(): void {
    setIsSetupOpen(false);
  }

  function finishSetup(): void {
    setIsSetupOpen(false);
    inputRef.current?.focus();
  }

  function activate(): void {
    if (needsSetup) {
      setIsSetupOpen((isOpen: boolean) => !isOpen);
      return;
    }
    dictation.toggle();
  }

  function renderCapsule(): ReactElement | null {
    if (phase === 'recording') {
      return <LevelBars />;
    }
    if (phase === 'transcribing') {
      return <ProgressDots />;
    }
    return null;
  }

  const buttonClass = `voice-button__button${isActive ? ' voice-button__button--active' : ''}`;

  return (
    <div className="voice-button">
      {renderCapsule()}
      <div className="voice-button__anchor">
        <button
          type="button"
          className={buttonClass}
          aria-haspopup="dialog"
          aria-expanded={isSetupOpen}
          aria-pressed={phase === 'recording'}
          aria-label={tooltip}
          title={tooltip}
          disabled={isButtonDisabled}
          onClick={activate}
        >
          <MicrophoneIcon />
        </button>
        {isSetupOpen && (
          <VoiceSetup placement={setupPlacement} onClose={closeSetup} onReady={finishSetup} />
        )}
      </div>
    </div>
  );
}

function tooltipFor(
  phase: DictationPhase,
  isBlocked: boolean,
  modelState: VoiceModelState | null,
): string {
  if (isBlocked) {
    return 'Diktat läuft in einer anderen Eingabe';
  }
  if (phase === 'recording') {
    return 'Aufnahme beenden (Strg+M) · Esc verwirft das Diktat';
  }
  if (phase === 'transcribing') {
    return 'Erkenne den Rest … · Klick oder Esc verwirft das Diktat';
  }
  if (modelState !== null && modelState.kind === 'downloading') {
    return `Sprachmodell wird geladen … ${String(downloadPercent(modelState))} %`;
  }
  return 'Diktieren (Strg+M)';
}

/** Drei Balken, deren Höhe dem Pegel folgt. Der Pegel kommt als CSS-Variable, damit die Höhen in der
 *  Stylesheet-Datei stehen und „Bewegung reduziert“ sie dort festsetzen kann. */
function LevelBars(): ReactElement {
  const level: number = useVoiceStore((state) => state.level);
  const style: CSSProperties & Record<'--voice-level', string> = {
    '--voice-level': String(Math.min(1, Math.max(0, level))),
  };
  return (
    <span className="voice-button__capsule" style={style} aria-hidden="true">
      {BAR_NUMBERS.map((barNumber: number) => (
        <span
          key={barNumber}
          className={`voice-button__bar voice-button__bar--${String(barNumber)}`}
        />
      ))}
    </span>
  );
}

function ProgressDots(): ReactElement {
  return (
    <span className="voice-button__capsule" aria-hidden="true">
      {DOT_NUMBERS.map((dotNumber: number) => (
        <span key={dotNumber} className="voice-button__dot" />
      ))}
    </span>
  );
}

function MicrophoneIcon(): ReactElement {
  return (
    <svg
      width="15"
      height="15"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <rect x="5" y="1.2" width="4" height="7" rx="2" />
      <path d="M2.8 7.2a4.2 4.2 0 0 0 8.4 0M7 11.4V13" />
    </svg>
  );
}
