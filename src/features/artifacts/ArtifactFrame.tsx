import { useEffect, useRef, useState } from 'react';
import type { ReactElement, RefObject } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import type { Artifact } from '@/lib/bindings/Artifact';
import type { ColorScheme } from '@/lib/bindings/ColorScheme';
import { artifactUrl, saveArtifactState } from '@/lib/artifacts';
import { useSettingsStore } from '@/stores/settings';
import { parseOpenRequest, stateFromFrame, stateMark } from '@/features/artifacts/artifactBridge';

interface ArtifactFrameProps {
  /** Session für den Befehl, der den Zustand speichert. */
  sessionId: string;
  /** `baseUrl` der Liste. */
  baseUrl: string;
  /** Nur Artefakte mit `viewable`; die übrigen zeigt die Rest-Karte. */
  item: Artifact;
  /** Erhöht sich bei „Neu laden“; neuer Schlüssel = neues iframe. */
  reloadToken: number;
  /** Erhöht sich nach „Zustand zurücksetzen“: Die Datei ist dann weg, ohne dass ein anderer sie änderte. */
  resetToken?: number;
  /** Vorschau und Vollbild gestalten den Rahmen verschieden. */
  className?: string;
  frameRef?: RefObject<HTMLIFrameElement | null>;
}

// Ohne `allow-same-origin`, `allow-popups` und `allow-top-navigation`: die Seite läuft als eigene, leere
// Herkunft und erreicht weder den Verwalter noch andere Fenster. Kein `srcdoc` — das erbte die App-CSP.
// Betrachter-Seiten (ADR 047) laufen im selben Rahmen wie HTML.
const FRAME_SANDBOX = 'allow-scripts allow-forms allow-modals';

// Die Seite kann die Nachricht auch ohne Klick schicken; mehr als ein Browser-Tab je Sekunde wäre Spam.
const OPEN_INTERVAL_MS = 1000;

// Die Abfrage zeigt nur den neuesten Zeitpunkt des Zustands; die letzten eigenen genügen (Critical Rule 3).
const OWN_STATE_ATS = 8;

/** Ein Stand der Seite und seine Prüfsumme (`null` ohne). */
interface StateReport {
  state: Record<string, string>;
  mark: string | null;
}

export function ArtifactFrame({
  sessionId,
  baseUrl,
  item,
  reloadToken,
  resetToken = 0,
  className = 'artifacts__frame',
  frameRef,
}: ArtifactFrameProps): ReactElement {
  const ownFrame = useRef<HTMLIFrameElement>(null);
  const frame: RefObject<HTMLIFrameElement | null> = frameRef ?? ownFrame;
  const lastOpenedAt = useRef<number>(0);
  // Zustand (ADR 081): erhöht sich, wenn ein anderer ihn änderte; neuer Schlüssel = neues iframe.
  const [stateReloads, setStateReloads] = useState<number>(0);
  const knownStateAt = useRef<number | undefined>(item.stateModifiedAt);
  const ownStateAts = useRef<number[]>([]);
  const knownResets = useRef<number>(resetToken);
  const saves = useRef<{ isRunning: boolean; next: StateReport | null }>({
    isRunning: false,
    next: null,
  });
  const stateAt: number | undefined = item.stateModifiedAt;
  // Der Rahmen sieht nur das Schema des Systems; das der App geht als Parameter in die Adresse.
  const colorScheme: ColorScheme = useSettingsStore(
    (state) => state.settings?.colorScheme ?? 'system',
  );

  // Links nach außen meldet die Brücke im Rahmen (`bridge.js`); geöffnet wird im Standardbrowser.
  useEffect(() => {
    function openLink(event: MessageEvent<unknown>): void {
      if (event.source === null || event.source !== frame.current?.contentWindow) return;
      const url: string | null = parseOpenRequest(event.data);
      const now: number = Date.now();
      if (url === null || now - lastOpenedAt.current < OPEN_INTERVAL_MS) return;
      lastOpenedAt.current = now;
      openUrl(url).catch((error: unknown): void => {
        console.error('Link aus dem Artefakt nicht geöffnet', error);
      });
    }
    window.addEventListener('message', openLink);
    return (): void => {
      window.removeEventListener('message', openLink);
    };
  }, [frame]);

  // Die Seite meldet ihren Zustand bei jeder Änderung. Die Datei bestimmt `item`, die eigene Auswahl, nie die
  // Nachricht. Läuft ein Speichern, wartet nur der neueste Stand; so bleibt die Reihenfolge und die Zahl der
  // Aufrufe klein. Wartet danach nichts mehr, ist der neueste Stand gespeichert: Der Rahmen bestätigt ihn der
  // Seite, die dann ihren Puffer in `window.name` leert.
  useEffect(() => {
    const file: string = item.file;
    function save(report: StateReport): void {
      saves.current.isRunning = true;
      saveArtifactState(sessionId, file, report.state)
        .then((modifiedAt: number): void => {
          ownStateAts.current = [...ownStateAts.current, modifiedAt].slice(-OWN_STATE_ATS);
          if (saves.current.next === null && report.mark !== null) {
            // Ohne eigene Herkunft (`sandbox`) gibt es kein Ziel außer `*`; die Prüfsumme verrät nichts.
            frame.current?.contentWindow?.postMessage(
              { verwalter: 1, type: 'state-saved', mark: report.mark },
              '*',
            );
          }
        })
        .catch((error: unknown): void => {
          console.error('Zustand des Artefakts nicht gespeichert', error);
        })
        .finally((): void => {
          const next: StateReport | null = saves.current.next;
          saves.current.next = null;
          if (next === null) {
            saves.current.isRunning = false;
          } else {
            save(next);
          }
        });
    }
    function onState(event: MessageEvent<unknown>): void {
      const state: Record<string, string> | null = stateFromFrame(
        event.source,
        frame.current?.contentWindow,
        event.data,
      );
      if (state === null) return;
      const report: StateReport = { state, mark: stateMark(event.data) };
      if (saves.current.isRunning) {
        saves.current.next = report;
      } else {
        save(report);
      }
    }
    window.addEventListener('message', onState);
    return (): void => {
      window.removeEventListener('message', onState);
    };
  }, [frame, item.file, sessionId]);

  // Nach „Zustand zurücksetzen“ lädt der Rahmen über `reloadToken`; dass die Abfrage danach keine Datei mehr zeigt,
  // ist kein Grund für ein zweites Laden.
  useEffect(() => {
    if (resetToken === knownResets.current) return;
    knownResets.current = resetToken;
    knownStateAt.current = undefined;
  }, [resetToken]);

  // Ändert sich die Zustandsdatei und stammt das nicht vom eigenen Speichern, lädt der Rahmen neu.
  useEffect(() => {
    if (stateAt === knownStateAt.current) return;
    knownStateAt.current = stateAt;
    if (stateAt !== undefined && ownStateAts.current.includes(stateAt)) return;
    // Ein laufendes Speichern kennt seinen Zeitpunkt erst, wenn es antwortet; die nächste Abfrage zeigt ihn.
    if (saves.current.isRunning) return;
    setStateReloads((count: number) => count + 1);
  }, [stateAt]);

  return (
    <iframe
      key={`${item.file}:${String(reloadToken)}:${String(stateReloads)}`}
      ref={frame}
      className={className}
      src={artifactUrl(baseUrl, item, colorScheme)}
      sandbox={FRAME_SANDBOX}
      allow="fullscreen"
      title={item.title}
    />
  );
}
