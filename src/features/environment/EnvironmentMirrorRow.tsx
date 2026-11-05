import type { ReactElement } from 'react';
import {
  hintCount,
  isPending,
  kindLabel,
  mirroredText,
  mirrorSummary,
  stateText,
} from '@/features/environment/environmentTexts';
import { useEnvironment } from '@/features/environment/useEnvironment';
import { SettingRow } from '@/features/settings/SettingRow';
import type { EnvironmentStatus } from '@/lib/bindings/EnvironmentStatus';
import type { MirrorDiff } from '@/lib/bindings/MirrorDiff';
import './EnvironmentRows.css';

function renderList(diffs: readonly MirrorDiff[]): ReactElement {
  return (
    <ul className="environment-row__list">
      {diffs.map((diff: MirrorDiff) => (
        <li key={`${diff.kind}/${diff.name}`} className="environment-row__item">
          <span className="environment-row__kind">{kindLabel(diff.kind)}</span>
          <span>{diff.name}</span>
          <span
            className={`environment-row__state${isPending(diff) ? ' environment-row__state--pending' : ''}`}
          >
            {stateText(diff)}
          </span>
        </li>
      ))}
    </ul>
  );
}

export function EnvironmentMirrorRow(): ReactElement {
  const { status, mirrorError, isChecking, isMirroring, check, mirror } = useEnvironment();

  function renderCheckButton(): ReactElement {
    return (
      <button type="button" className="settings-view__button" disabled={isChecking} onClick={check}>
        Erneut prüfen
      </button>
    );
  }

  function renderEqual(loaded: EnvironmentStatus): ReactElement {
    const hints: MirrorDiff[] = loaded.diffs.filter((diff: MirrorDiff) => !isPending(diff));
    const equalText: string =
      loaded.lastMirror === null ? 'Gleich' : `Gleich · ${mirroredText(loaded.lastMirror)}`;
    return (
      <>
        <div className="environment-row__line">
          <span className="environment-row__ok">{equalText}</span>
          {renderCheckButton()}
        </div>
        {hints.length > 0 && (
          <details className="environment-row__hints">
            <summary>{hintCount(hints.length)}</summary>
            {renderList(hints)}
          </details>
        )}
      </>
    );
  }

  function renderDiffering(loaded: EnvironmentStatus, summary: string): ReactElement {
    return (
      <>
        <div className="environment-row__line">
          <span className="environment-row__warn">{summary}</span>
          <button
            type="button"
            className="settings-view__button environment-row__primary"
            disabled={isMirroring}
            onClick={mirror}
          >
            {isMirroring ? 'Spiegelt …' : 'Spiegeln'}
          </button>
        </div>
        {renderList(loaded.diffs)}
      </>
    );
  }

  function renderState(): ReactElement {
    if (status === null || status.masterDir === null) {
      return <span className="environment-row__muted">{isChecking ? 'Lädt …' : '—'}</span>;
    }
    if (status.masterError !== null) {
      return (
        <span className="environment-row__muted">
          Kein Abgleich, solange der Master-Ordner ungültig ist.
        </span>
      );
    }
    const summary: string | null = mirrorSummary(status.diffs);
    if (summary === null) {
      return renderEqual(status);
    }
    return renderDiffering(status, summary);
  }

  return (
    <SettingRow id="environmentMirror">
      <div className="environment-row__stack">
        {renderState()}
        {mirrorError !== null && <span className="environment-row__warn">{mirrorError}</span>}
      </div>
    </SettingRow>
  );
}
