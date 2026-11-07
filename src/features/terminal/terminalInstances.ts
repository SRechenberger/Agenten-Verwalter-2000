import { FitAddon } from '@xterm/addon-fit';
import { Terminal } from '@xterm/xterm';
import '@xterm/xterm/css/xterm.css';
import { terminalFont, terminalTheme } from '@/features/terminal/terminalTheme';
import type { TerminalOutput } from '@/lib/bindings/TerminalOutput';
import { isProjectLockedNow } from '@/features/remote/useProjectAccess';
import { attachTerminal, detachTerminal, resizeTerminal, writeTerminal } from '@/lib/terminal';
import { useSettingsStore } from '@/stores/settings';

const FONT_SIZE = 13;
const SCROLLBACK_LINES = 5000;
const RESIZE_DEBOUNCE_MS = 50;
const IS_MAC: boolean = navigator.platform.toUpperCase().includes('MAC');

export interface TerminalInstance {
  projectId: string;
  term: Terminal;
  fit: FitAddon;
  element: HTMLDivElement;
  isOpened: boolean;
  isDisposed: boolean;
  lastCols: number;
  lastRows: number;
  resizeTimer: number | null;
}

/** Jede Instanz lebt hier von ihrem ersten Anzeigen bis zum Ende ihres Terminals — nicht in React: Ausblenden und
 *  Wechsel hängen nur das DOM-Element aus und ein, damit Inhalt, Scrollstand und Vollbild-Programme stehen bleiben
 *  (ADR 055). */
const instances = new Map<string, TerminalInstance>();

function writeOutput(term: Terminal, message: TerminalOutput): void {
  if (message.kind === 'data') {
    term.write(message.text);
    return;
  }
  const suffix: string = message.code === null ? '' : `, Code ${String(message.code)}`;
  term.write(`\r\n[Prozess beendet${suffix}]\r\n`);
}

/** Verdrahtet den Kanal: Ausgabe, die vor dem Puffer eintrifft, wartet und läuft danach in Reihenfolge. */
function wireOutput(terminalId: string, instance: TerminalInstance): void {
  const pending: TerminalOutput[] = [];
  let isReady = false;
  attachTerminal(terminalId, (message: TerminalOutput) => {
    if (instance.isDisposed) {
      return;
    }
    if (!isReady) {
      pending.push(message);
      return;
    }
    writeOutput(instance.term, message);
  })
    .then((snapshot: string) => {
      if (instance.isDisposed) {
        return;
      }
      instance.term.write(snapshot);
      for (const message of pending) {
        writeOutput(instance.term, message);
      }
      pending.length = 0;
      isReady = true;
    })
    .catch((reason: unknown) => {
      console.error('Terminal nicht angehängt', reason);
    });
}

function wireKeys(term: Terminal): void {
  term.attachCustomKeyEventHandler((event: KeyboardEvent): boolean => {
    if (event.type !== 'keydown') {
      return true;
    }
    const key: string = event.key.toLowerCase();
    const hasCommandKey: boolean = IS_MAC ? event.metaKey : event.ctrlKey;
    if (hasCommandKey && key === 'c' && term.hasSelection()) {
      navigator.clipboard.writeText(term.getSelection()).catch((reason: unknown) => {
        console.error('Auswahl nicht kopiert', reason);
      });
      return false;
    }
    // Das native `paste`-Ereignis verarbeitet xterm.js selbst; die Taste darf es nicht zusätzlich als Eingabe senden.
    if (hasCommandKey && key === 'v') {
      return false;
    }
    // Geht an das globale Kürzel in App.tsx.
    if (event.ctrlKey && key === 'ö') {
      return false;
    }
    return true;
  });
}

export function acquire(terminalId: string, projectId: string): TerminalInstance {
  const existing: TerminalInstance | undefined = instances.get(terminalId);
  if (existing !== undefined) {
    return existing;
  }
  const element: HTMLDivElement = document.createElement('div');
  element.className = 'terminal-pane__screen';
  const term = new Terminal({
    fontFamily: terminalFont(),
    fontSize: FONT_SIZE,
    cursorBlink: true,
    scrollback: SCROLLBACK_LINES,
    theme: terminalTheme(),
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  const instance: TerminalInstance = {
    projectId,
    term,
    fit,
    element,
    isOpened: false,
    isDisposed: false,
    lastCols: 0,
    lastRows: 0,
    resizeTimer: null,
  };
  term.onData((data: string) => {
    // Der Host liest nur mit, solange ein Gast steuert; der Core lehnte die Eingabe ohnehin ab.
    if (isProjectLockedNow(projectId)) {
      return;
    }
    writeTerminal(terminalId, data).catch((reason: unknown) => {
      console.error('Eingabe nicht gesendet', reason);
    });
  });
  wireKeys(term);
  wireOutput(terminalId, instance);
  instances.set(terminalId, instance);
  return instance;
}

/** Hängt das Element in den Host; beim ersten Mal öffnet es xterm darin. */
export function show(terminalId: string, instance: TerminalInstance, host: HTMLElement): void {
  host.appendChild(instance.element);
  if (!instance.isOpened) {
    instance.term.open(instance.element);
    instance.isOpened = true;
  }
  fitAndResize(terminalId);
  instance.term.refresh(0, instance.term.rows - 1);
  instance.term.focus();
}

export function hide(instance: TerminalInstance): void {
  instance.element.remove();
}

export function fitAndResize(terminalId: string): void {
  const instance: TerminalInstance | undefined = instances.get(terminalId);
  if (
    instance === undefined ||
    !instance.isOpened ||
    instance.element.clientWidth === 0 ||
    instance.element.clientHeight === 0
  ) {
    return;
  }
  instance.fit.fit();
  const { cols, rows } = instance.term;
  if (cols === instance.lastCols && rows === instance.lastRows) {
    return;
  }
  instance.lastCols = cols;
  instance.lastRows = rows;
  if (instance.resizeTimer !== null) {
    window.clearTimeout(instance.resizeTimer);
  }
  instance.resizeTimer = window.setTimeout(() => {
    instance.resizeTimer = null;
    if (isProjectLockedNow(instance.projectId)) {
      return;
    }
    resizeTerminal(terminalId, cols, rows).catch((reason: unknown) => {
      console.error('Größe nicht gemeldet', reason);
    });
  }, RESIZE_DEBOUNCE_MS);
}

function destroy(terminalId: string): void {
  const instance: TerminalInstance | undefined = instances.get(terminalId);
  if (instance === undefined) {
    return;
  }
  instance.isDisposed = true;
  if (instance.resizeTimer !== null) {
    window.clearTimeout(instance.resizeTimer);
  }
  detachTerminal(terminalId).catch((reason: unknown) => {
    console.error('Terminal nicht gelöst', reason);
  });
  instance.term.dispose();
  instance.element.remove();
  instances.delete(terminalId);
}

/** Räumt jede Instanz des Vorhabens ab, deren Terminal nicht mehr in `ids` steht (geschlossen, archiviert). */
export function prune(projectId: string, ids: readonly string[]): void {
  for (const [terminalId, instance] of [...instances]) {
    if (instance.projectId === projectId && !ids.includes(terminalId)) {
      destroy(terminalId);
    }
  }
}

function applyThemeToAll(): void {
  window.requestAnimationFrame(() => {
    for (const instance of instances.values()) {
      instance.term.options.theme = terminalTheme();
    }
  });
}

// Einmal beim Laden des Moduls: der Farbschema-Wechsel (Einstellung oder Windows) färbt alle Instanzen um.
useSettingsStore.subscribe((state, previous) => {
  if (state.settings?.colorScheme !== previous.settings?.colorScheme) {
    applyThemeToAll();
  }
});
window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyThemeToAll);
