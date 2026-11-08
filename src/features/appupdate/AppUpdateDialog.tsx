import { useState } from 'react';
import type { ReactElement } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { Dialog } from '@/components/Dialog';
import { ExternalLink } from '@/components/ExternalLink';
import { Markdown } from '@/components/Markdown';
import {
  formatReleaseDate,
  installNote,
  progressFraction,
  progressText,
  runningSessionsWarning,
} from '@/features/appupdate/appUpdateTexts';
import { useAppUpdateInstall } from '@/features/appupdate/useAppUpdate';
import { updateWarning } from '@/features/remote/remoteTexts';
import { useConnectedGuests } from '@/features/remote/useProjectAccess';
import type { AppUpdateStatus } from '@/lib/bindings/AppUpdateStatus';
import type { ReleaseInfo } from '@/lib/bindings/ReleaseInfo';
import { commandErrorText } from '@/lib/errors';
import './AppUpdateDialog.css';

const TITLE_ID = 'app-update-title';

interface AppUpdateDialogProps {
  status: AppUpdateStatus;
  /** Sessions, die der Neustart mitten in der Arbeit unterbräche. */
  runningSessions: number;
  onClose: () => void;
}

/** Release-Notes aller neueren Versionen und der Knopf, der die neueste installiert (ADR 041). */
export function AppUpdateDialog({
  status,
  runningSessions,
  onClose,
}: AppUpdateDialogProps): ReactElement {
  const { progress, isInstalling, error, install } = useAppUpdateInstall();
  // Der Neustart trennt die Gäste und beendet die Agenten, die für sie laufen.
  const guests = useConnectedGuests();
  const [openError, setOpenError] = useState<string | null>(null);
  const isManual: boolean = status.installKind === 'manual';
  const latestRelease: ReleaseInfo | undefined = status.releases[0];

  // Während der Installation schließt weder Esc noch ein Klick daneben.
  function handleClose(): void {
    if (!isInstalling) {
      onClose();
    }
  }

  function openReleasePage(): void {
    if (latestRelease === undefined) {
      return;
    }
    setOpenError(null);
    openUrl(latestRelease.url).catch((reason: unknown) => {
      console.error('Release-Seite nicht geöffnet', reason);
      setOpenError(`Release-Seite nicht geöffnet: ${commandErrorText(reason)}`);
    });
  }

  function renderRelease(release: ReleaseInfo): ReactElement {
    const date: string = formatReleaseDate(release.publishedAt);
    return (
      <article key={release.version} className="app-update-dialog__release">
        <header className="app-update-dialog__release-header">
          <h4 className="app-update-dialog__release-version">Version {release.version}</h4>
          {date !== '' && <span className="app-update-dialog__release-date">{date}</span>}
        </header>
        {release.notes === '' ? (
          <p className="app-update-dialog__empty">Keine Release-Notes.</p>
        ) : (
          <Markdown text={release.notes} />
        )}
        <ExternalLink href={release.url}>Auf GitHub ansehen</ExternalLink>
      </article>
    );
  }

  function renderProgress(): ReactElement | null {
    if (!isInstalling) {
      return null;
    }
    const percent: number = Math.round(progressFraction(progress) * 100);
    return (
      <div className="app-update-dialog__progress" role="status">
        <div
          className="app-update-dialog__bar"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent}
        >
          <div className="app-update-dialog__bar-fill" style={{ width: `${String(percent)}%` }} />
        </div>
        <span className="app-update-dialog__progress-text">{progressText(progress)}</span>
      </div>
    );
  }

  function renderErrors(): ReactElement[] {
    const sentences: string[] = [];
    if (error !== null) {
      sentences.push(error);
    }
    if (openError !== null) {
      sentences.push(openError);
    }
    return sentences.map((sentence: string) => (
      <p key={sentence} className="app-update-dialog__error" role="alert">
        {sentence}
      </p>
    ));
  }

  function renderPrimaryAction(): ReactElement {
    if (isManual) {
      return (
        <button
          type="button"
          className="app-update-dialog__button app-update-dialog__button--primary"
          onClick={openReleasePage}
        >
          Release-Seite öffnen
        </button>
      );
    }
    return (
      <button
        type="button"
        className="app-update-dialog__button app-update-dialog__button--primary"
        disabled={isInstalling}
        onClick={install}
      >
        {runningSessions > 0 || guests.length > 0 ? 'Trotzdem installieren' : 'Jetzt installieren'}
      </button>
    );
  }

  return (
    <Dialog labelledBy={TITLE_ID} className="app-update-dialog" onClose={handleClose}>
      <h3 id={TITLE_ID} className="app-update-dialog__title">
        Neue Version {status.latest ?? ''}
      </h3>
      <div className="app-update-dialog__releases">{status.releases.map(renderRelease)}</div>
      <p className="app-update-dialog__note">
        {installNote(status.installKind, status.targetPath)}
      </p>
      {runningSessions > 0 && !isManual && (
        <p className="app-update-dialog__warning">{runningSessionsWarning(runningSessions)}</p>
      )}
      {guests.length > 0 && !isManual && (
        <p className="app-update-dialog__warning">{updateWarning(guests)}</p>
      )}
      {renderProgress()}
      {renderErrors()}
      <div className="app-update-dialog__actions">
        <button
          type="button"
          className="app-update-dialog__button"
          disabled={isInstalling}
          onClick={onClose}
        >
          Später
        </button>
        {renderPrimaryAction()}
      </div>
    </Dialog>
  );
}
