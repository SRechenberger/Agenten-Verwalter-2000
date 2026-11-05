import { Channel, invoke } from '@tauri-apps/api/core';
import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';
import type { TerminalChanged } from '@/lib/bindings/TerminalChanged';
import type { TerminalInfo } from '@/lib/bindings/TerminalInfo';
import type { TerminalOutput } from '@/lib/bindings/TerminalOutput';

const TERMINAL_CHANGED_EVENT = 'terminal://changed';

/** Die Terminals des Vorhabens, nach Nummer.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} wenn der Core ablehnt */
export function listTerminals(projectId: string): Promise<TerminalInfo[]> {
  return invoke<TerminalInfo[]>('terminal_list', { projectId });
}

/** Startet die Shell des Betriebssystems im Workspace des Vorhabens.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `terminal` (schon 8 laufende, Start gescheitert) */
export function openTerminal(projectId: string, cols: number, rows: number): Promise<TerminalInfo> {
  return invoke<TerminalInfo>('terminal_open', { projectId, cols, rows });
}

/** Hängt den Kanal an und liefert den Puffer (bis 1 MiB); ist die Shell beendet, folgt sofort `exit`.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `terminal` (unbekannte ID) */
export function attachTerminal(
  id: string,
  onOutput: (message: TerminalOutput) => void,
): Promise<string> {
  const channel = new Channel<TerminalOutput>();
  channel.onmessage = onOutput;
  return invoke<string>('terminal_attach', { id, onOutput: channel });
}

/** Löst den Kanal; eine unbekannte ID bleibt still. */
export async function detachTerminal(id: string): Promise<void> {
  await invoke('terminal_detach', { id });
}

/** Schickt Eingabe ins Pseudo-Terminal. */
export async function writeTerminal(id: string, data: string): Promise<void> {
  await invoke('terminal_write', { id, data });
}

/** Meldet die neue Größe; wirkt nur auf eine laufende Shell. */
export async function resizeTerminal(id: string, cols: number, rows: number): Promise<void> {
  await invoke('terminal_resize', { id, cols, rows });
}

/** Beendet den Prozess und entfernt das Terminal. */
export async function closeTerminal(id: string): Promise<void> {
  await invoke('terminal_close', { id });
}

/** Meldet jedes neue und jedes entfernte Terminal samt Vorhaben. */
export function onTerminalChanged(callback: (event: TerminalChanged) => void): Promise<UnlistenFn> {
  return listen<TerminalChanged>(TERMINAL_CHANGED_EVENT, (event: Event<TerminalChanged>) => {
    callback(event.payload);
  });
}
