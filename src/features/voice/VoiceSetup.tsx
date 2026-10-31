import { useEffect } from 'react';
import type { ReactElement } from 'react';
import { Popover } from '@/components/Popover';
import { downloadPercent, formatMegabytes } from '@/features/voice/downloadProgress';
import type { DownloadingState } from '@/features/voice/downloadProgress';
import { commandErrorText } from '@/lib/errors';
import { cancelVoiceModelDownload, downloadVoiceModel } from '@/lib/voice';
import { useVoiceStore } from '@/stores/voice';
import './VoiceSetup.css';

const SETUP_WIDTH = 300;
const MODEL_SIZE_LABEL = '670 MB';

interface VoiceSetupProps {
  placement: 'above' | 'below';
  onClose: () => void;
  /** Das Modell ist fertig; der Aufrufer schließt das Menü und gibt den Fokus zurück. */
  onReady: () => void;
}

/** Inhalt des Menüs am Mikrofon, solange das Sprachmodell fehlt oder geladen wird. */
export function VoiceSetup({ placement, onClose, onReady }: VoiceSetupProps): ReactElement {
  const modelState = useVoiceStore((state) => state.modelState);
  const downloadError: string | null = useVoiceStore((state) => state.downloadError);
  const setDownloadError = useVoiceStore((state) => state.setDownloadError);
  const isReady: boolean = modelState?.kind === 'ready';

  useEffect(() => {
    if (isReady) {
      onReady();
    }
  }, [isReady, onReady]);

  function startDownload(): void {
    setDownloadError(null);
    downloadVoiceModel().catch((reason: unknown) => {
      setDownloadError(commandErrorText(reason));
    });
  }

  function cancelDownload(): void {
    cancelVoiceModelDownload().catch((reason: unknown) => {
      console.error('Download nicht abbrechbar', commandErrorText(reason));
    });
  }

  function renderProgress(downloading: DownloadingState): ReactElement {
    return (
      <>
        <progress
          className="voice-setup__progress"
          max={downloading.totalBytes}
          value={downloading.receivedBytes}
        />
        <p className="voice-setup__status">
          {downloadPercent(downloading)} % · {formatMegabytes(downloading.receivedBytes)} von{' '}
          {formatMegabytes(downloading.totalBytes)} MB
        </p>
        <button type="button" className="voice-setup__secondary" onClick={cancelDownload}>
          Abbrechen
        </button>
      </>
    );
  }

  function renderStart(): ReactElement {
    return (
      <>
        {downloadError !== null && (
          <p className="voice-setup__error" role="alert">
            {downloadError}
          </p>
        )}
        <button type="button" className="voice-setup__primary" onClick={startDownload}>
          {downloadError === null ? 'Herunterladen' : 'Erneut versuchen'}
        </button>
      </>
    );
  }

  return (
    <Popover
      label="Diktieren einrichten"
      placement={placement}
      align="end"
      width={SETUP_WIDTH}
      onClose={onClose}
    >
      <div className="voice-setup">
        <h2 className="voice-setup__title">Diktieren einrichten</h2>
        <p className="voice-setup__text">
          Für das Diktieren wird einmalig ein Sprachmodell ({MODEL_SIZE_LABEL}) auf deinen Rechner
          geladen. Danach läuft die Erkennung ohne Internet — deine Aufnahme verlässt den Rechner
          nie.
        </p>
        {modelState?.kind === 'downloading' ? renderProgress(modelState) : renderStart()}
      </div>
    </Popover>
  );
}
