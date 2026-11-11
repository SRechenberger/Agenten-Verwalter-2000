// Die Version des Verwalters an ihren vier Stellen: prüfen, lesen, setzen, und die nächste aus den Annotationen seit dem letzten Release errechnen.
// Läuft mit `node` direkt: Node streift nur die Typen ab, also kein Enum und kein `namespace`.
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

interface Place {
  file: string;
  // Gruppe 1 ist alles vor dem Wert, Gruppe 2 der Wert; ohne `g`, es zählt nur der erste Treffer.
  pattern: RegExp;
}

interface Case {
  base: string;
  subjects: string[];
  expected: string | undefined;
}

const PACKAGE: Place = { file: 'package.json', pattern: /^( {2}"version": ")([^"\n]*)"/m };
const PLACES: Place[] = [
  PACKAGE,
  { file: 'src-tauri/tauri.conf.json', pattern: /^( {2}"version": ")([^"\n]*)"/m },
  {
    file: 'src-tauri/Cargo.toml',
    pattern: /^(\[package\]\n(?:(?!\[).*\n)*?version = ")([^"\n]*)"/m,
  },
  { file: 'src-tauri/Cargo.lock', pattern: /^(name = "verwalter"\nversion = ")([^"\n]*)"/m },
];
const VERSION = /^\d+\.\d+\.\d+$/;
const USAGE = 'Aufruf: node scripts/version.ts check | get | bump <X.Y.Z> | next <ref> | selftest';
const ROOT = join(import.meta.dirname, '..');

// Die Fälle des Selbsttests, die Nummer ist ihr Index plus eins.
const CASES: Case[] = [
  { base: '0.73.0', subjects: ['fix(chat): x [PATCH] (#41)'], expected: '0.73.1' },
  { base: '0.73.1', subjects: ['a [PATCH]', 'b [MINOR]'], expected: '0.74.0' },
  { base: '0.73.1', subjects: ['a [MAJOR]', 'b [MINOR]', 'c [PATCH]'], expected: '0.74.0' },
  { base: '1.2.3', subjects: ['a [MAJOR]', 'b [PATCH]'], expected: '2.0.0' },
  { base: '1.2.3', subjects: ['a [MINOR]'], expected: '1.3.0' },
  {
    base: '1.2.3',
    subjects: ['Merge pull request #9 from x/release/v1.2.3', 'chore(release): Version 1.2.3'],
    expected: undefined,
  },
  {
    base: '1.2.3',
    subjects: ['fix: PATCH-Aufruf an Taiga', 'feat: y [minor]'],
    expected: undefined,
  },
  { base: '0.73.1', subjects: ['a [MAJOR]', 'b [PATCH]'], expected: '0.74.0' },
];

function fail(message: string): never {
  console.error(message);
  process.exit(1);
}

function path(place: Place): string {
  return join(ROOT, place.file);
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

// Eine Betreffzeile trägt `[MAJOR]`, `[MINOR]` oder `[PATCH]`, großgeschrieben und in eckigen Klammern, an beliebiger Stelle:
// GitHub hängt ` (#N)` an den Betreff. Ein nacktes „PATCH“ zählt nicht, die Commit-Texte sprechen von HTTP-PATCH (Taiga).
// Dieselbe Regel prüft `.github/workflows/annotation.yml` im PR-Titel.
// MAJOR bei 0.y.z hebt nur den Minor: 1.0.0 entscheidet der User ausdrücklich.
function nextVersion(base: string, subjects: string[]): string | undefined {
  const [major, minor, patch] = base.split('.').map(Number);
  const has = (kind: string): boolean =>
    subjects.some((subject: string): boolean => subject.includes(`[${kind}]`));
  if (has('MAJOR')) {
    return (major === 0 ? [0, minor + 1, 0] : [major + 1, 0, 0]).join('.');
  }
  if (has('MINOR')) {
    return [major, minor + 1, 0].join('.');
  }
  return has('PATCH') ? [major, minor, patch + 1].join('.') : undefined;
}

function git(args: string[]): string {
  try {
    return execFileSync('git', args, {
      cwd: ROOT,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    });
  } catch (error) {
    return fail(error instanceof Error ? error.message : String(error));
  }
}

// Die nächste Version aus der Version in `<ref>:package.json` und den Betreffzeilen seit `<ref>`.
function next(ref: string): string | undefined {
  const base = PACKAGE.pattern.exec(git(['show', `${ref}:package.json`]))?.[2];
  if (base === undefined || !VERSION.test(base)) {
    fail(`${ref}:package.json trägt keine Version der Form X.Y.Z: "${base ?? 'fehlt'}"`);
  }
  return nextVersion(base, git(['log', '--format=%s', `${ref}..HEAD`]).split('\n'));
}

function selftest(): void {
  let failed = false;
  for (const [index, { base, subjects, expected }] of CASES.entries()) {
    const actual = nextVersion(base, subjects);
    const nr = String(index + 1);
    if (actual === expected) {
      console.log(`PASS ${nr}`);
    } else {
      failed = true;
      console.log(`FAIL ${nr}: erwartet ${expected ?? 'keine'}, bekommen ${actual ?? 'keine'}`);
    }
  }
  if (failed) {
    process.exit(1);
  }
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
  case 'next': {
    if (value === '') {
      fail(USAGE);
    }
    const version = next(value);
    if (version === undefined) {
      fail(
        `Seit ${value} gibt es nichts zu releasen: kein Commit mit [MAJOR], [MINOR] oder [PATCH].`,
      );
    }
    console.log(version);
    break;
  }
  case 'selftest':
    selftest();
    break;
  default:
    fail(USAGE);
}
