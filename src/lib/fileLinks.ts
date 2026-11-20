import { invoke } from '@tauri-apps/api/core';

/** Gespiegelt aus `file_links::OPENABLE_EXTENSIONS` in `src-tauri/src/file_links/mod.rs` (Rust ist maßgeblich).
 *  Hier entscheidet die Liste nur, was als Link erscheint; ob geöffnet wird, prüft der Core. */
export const OPENABLE_EXTENSIONS: readonly string[] = [
  'html',
  'htm',
  'pdf',
  'svg',
  'png',
  'jpg',
  'jpeg',
  'gif',
  'webp',
  'md',
  'txt',
];

const FILE_URL_PREFIX = 'file:///';
const MAX_PATH_LENGTH = 400;
const LINE_NUMBER_SUFFIX = /(:\d+){1,2}$/;

/** Pfad, wenn `text` als Ganzes ein Dateiverweis ist, sonst `null`. Die angehängte Zeilennummer bleibt
 *  im Ergebnis stehen (der Core entfernt sie), damit Anzeige und Titel dem Original entsprechen. */
export function filePathOf(text: string): string | null {
  const trimmed: string = text.trim();
  if (trimmed === '' || trimmed.includes('\n') || trimmed.length > MAX_PATH_LENGTH) {
    return null;
  }
  let withoutScheme: string = trimmed;
  if (trimmed.toLowerCase().startsWith(FILE_URL_PREFIX)) {
    withoutScheme = trimmed.slice(FILE_URL_PREFIX.length);
  }
  if (withoutScheme.includes('://')) {
    return null;
  }
  const withoutLineNumber: string = withoutScheme.replace(LINE_NUMBER_SUFFIX, '');
  const lastDot: number = withoutLineNumber.lastIndexOf('.');
  const lastSeparator: number = Math.max(
    withoutLineNumber.lastIndexOf('/'),
    withoutLineNumber.lastIndexOf('\\'),
  );
  if (lastDot <= lastSeparator + 1) {
    return null;
  }
  const extension: string = withoutLineNumber.slice(lastDot + 1).toLowerCase();
  if (!OPENABLE_EXTENSIONS.includes(extension)) {
    return null;
  }
  return trimmed;
}

const WINDOWS_ABSOLUTE = /^[a-z]:[\\/]/i;

/** `file:///`-Adresse für „Link-Adresse kopieren“ und die Statusleiste; `undefined` bei relativen
 *  Pfaden (die brauchen den Core). Ohne eigene Adresse wäre es `http://tauri.localhost/#`. */
export function fileHrefOf(path: string): string | undefined {
  const withoutScheme: string = path.replace(/^file:\/\/\//i, '');
  if (!WINDOWS_ABSOLUTE.test(withoutScheme)) {
    return undefined;
  }
  return `${FILE_URL_PREFIX}${encodeURI(withoutScheme.replace(/\\/g, '/'))}`;
}

/** Öffnet die Datei mit dem Standardprogramm.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `fileNotFound`,
 *    `fileNotAllowed` (Dateityp oder außerhalb des Vorhabens), `io` (Öffnen gescheitert) */
export async function openFileLink(sessionId: string, path: string): Promise<void> {
  await invoke('file_link_open', { sessionId, path });
}
