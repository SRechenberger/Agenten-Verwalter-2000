// Die Version des Verwalters an ihren vier Stellen: prüfen, lesen, setzen, mit einer anderen vergleichen.
// Läuft mit `node` direkt: Node streift nur die Typen ab, also kein Enum und kein `namespace`.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

interface Place {
  file: string;
  // Gruppe 1 ist alles vor dem Wert, Gruppe 2 der Wert; ohne `g`, es zählt nur der erste Treffer.
  pattern: RegExp;
}

const PLACES: Place[] = [
  { file: 'package.json', pattern: /^( {2}"version": ")([^"\n]*)"/m },
  { file: 'src-tauri/tauri.conf.json', pattern: /^( {2}"version": ")([^"\n]*)"/m },
  {
    file: 'src-tauri/Cargo.toml',
    pattern: /^(\[package\]\n(?:(?!\[).*\n)*?version = ")([^"\n]*)"/m,
  },
  { file: 'src-tauri/Cargo.lock', pattern: /^(name = "verwalter"\nversion = ")([^"\n]*)"/m },
];
const VERSION = /^\d+\.\d+\.\d+$/;
const USAGE = 'Aufruf: node scripts/version.ts check | get | bump <X.Y.Z> | newer-than <X.Y.Z>';

function fail(message: string): never {
  console.error(message);
  process.exit(1);
}

function path(place: Place): string {
  return join(import.meta.dirname, '..', place.file);
}

function read(place: Place): string {
  return existsSync(path(place)) ? readFileSync(path(place), 'utf8') : '';
}

function currentVersion(): string {
  const values = PLACES.map(
    (place: Place): string | undefined => place.pattern.exec(read(place))?.[2],
  );
  const [first] = values;
  if (first === undefined || values.some((value: string | undefined): boolean => value !== first)) {
    const lines = PLACES.map(
      (place: Place, index: number): string => `  ${place.file}: ${values[index] ?? 'fehlt'}`,
    );
    fail(
      `Die Versionen weichen ab, pnpm bump <X.Y.Z> setzt alle vier Stellen:\n${lines.join('\n')}`,
    );
  }
  return first;
}

function requireVersion(value: string): string {
  return VERSION.test(value) ? value : fail(`Keine Version der Form X.Y.Z: "${value}"\n${USAGE}`);
}

function bump(version: string): void {
  const changes = PLACES.map((place: Place): { place: Place; text: string } => {
    const text = read(place);
    if (!place.pattern.test(text)) {
      fail(`Die Version fehlt in ${place.file}, keine Datei geändert.`);
    }
    return {
      place,
      text: text.replace(
        place.pattern,
        (_match: string, before: string): string => `${before}${version}"`,
      ),
    };
  });
  for (const { place, text } of changes) {
    writeFileSync(path(place), text);
  }
}

function isNewer(version: string, base: string): boolean {
  const parts = version.split('.').map(Number);
  const baseParts = base.split('.').map(Number);
  for (const [index, part] of parts.entries()) {
    if (part !== baseParts[index]) {
      return part > baseParts[index];
    }
  }
  return false;
}

const [, , command, value = ''] = process.argv;

switch (command) {
  case 'check':
    currentVersion();
    break;
  case 'get':
    console.log(currentVersion());
    break;
  case 'bump':
    bump(requireVersion(value));
    break;
  case 'newer-than': {
    const version = currentVersion();
    if (!isNewer(requireVersion(version), requireVersion(value))) {
      fail(`Die Version ${version} liegt nicht über ${value}: pnpm bump <X.Y.Z> hebt sie an.`);
    }
    break;
  }
  default:
    fail(USAGE);
}
