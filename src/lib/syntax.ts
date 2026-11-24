import { common, createLowlight } from 'lowlight';
import powershell from 'highlight.js/lib/languages/powershell';
import agda from './agdaLanguage.ts';

/** Eine Sprachliste für Chat (`Markdown.tsx`) und Diff: `common` kennt kein PowerShell und kein Agda. */
export const HIGHLIGHT_LANGUAGES = { ...common, powershell, agda };

const lowlight = createLowlight(HIGHLIGHT_LANGUAGES);

// Die hast-Typen sind keine direkte Abhängigkeit; sie kommen aus der Rückgabe von lowlight.
type HastRoot = ReturnType<typeof lowlight.highlight>;
type HastNode = HastRoot['children'][number];
type HastElement = Extract<HastNode, { type: 'element' }>;

/** Ein Stück Text einer Zeile; `className` ist die Klassenkette der Farbe oder `null` für die Grundfarbe. */
export interface SyntaxSegment {
  text: string;
  className: string | null;
}

export type SyntaxLine = readonly SyntaxSegment[];

// Muss zu `src/styles/syntax.css` passen: nur diese Klassen färben oder stylen etwas.
export const COLORED_CLASSES: ReadonlySet<string> = new Set([
  'hljs-keyword',
  'hljs-literal',
  'hljs-built_in',
  'hljs-tag',
  'hljs-name',
  'hljs-selector-tag',
  'hljs-bullet',
  'hljs-string',
  'hljs-regexp',
  'hljs-link',
  'hljs-comment',
  'hljs-quote',
  'hljs-meta',
  'hljs-number',
  'hljs-title',
  'hljs-selector-class',
  'hljs-selector-id',
  'hljs-section',
  'hljs-type',
  'hljs-variable',
  'hljs-attr',
  'hljs-attribute',
  'hljs-property',
  'hljs-params',
  'hljs-addition',
  'hljs-deletion',
  'hljs-emphasis',
  'hljs-strong',
]);

/** Sprache für lowlight aus der Dateiendung; `null`, wenn lowlight sie nicht kennt. */
export function languageOf(path: string): string | null {
  const fileName: string = path.slice(path.lastIndexOf('/') + 1);
  const dotIndex: number = fileName.lastIndexOf('.');
  if (dotIndex < 0) {
    return null;
  }
  const extension: string = fileName.slice(dotIndex + 1).toLowerCase();
  return lowlight.registered(extension) ? extension : null;
}

export function plainLine(text: string): SyntaxLine {
  return [{ text, className: null }];
}

/** Liefert genau `text.split('\n').length` Zeilen; ohne Sprache oder bei einem Fehler von lowlight ungefärbt. */
export function highlightLines(text: string, language: string | null): SyntaxLine[] {
  if (language === null) {
    return text.split('\n').map(plainLine);
  }
  let tree: HastRoot;
  try {
    tree = lowlight.highlight(language, text);
  } catch {
    return text.split('\n').map(plainLine);
  }
  let current: SyntaxSegment[] = [];
  const lines: SyntaxSegment[][] = [current];

  function appendText(value: string, className: string | null): void {
    const pieces: string[] = value.split('\n');
    pieces.forEach((piece: string, index: number) => {
      if (index > 0) {
        current = [];
        lines.push(current);
      }
      appendSegment(current, piece, className);
    });
  }

  function walk(nodes: readonly HastNode[], inherited: string | null): void {
    for (const node of nodes) {
      if (node.type === 'text') {
        appendText(node.value, inherited);
      } else if (node.type === 'element') {
        walk(node.children, coloredClassOf(node, inherited));
      }
    }
  }

  walk(tree.children, null);
  return lines;
}

export function highlightLine(text: string, language: string | null): SyntaxLine {
  return highlightLines(text, language)[0] ?? [];
}

/** Die Klassenkette des Elements, wenn sie etwas färbt; sonst gilt die des nächsten gefärbten äußeren Elements. */
function coloredClassOf(element: HastElement, inherited: string | null): string | null {
  const classNames: unknown = element.properties.className;
  if (!Array.isArray(classNames)) {
    return inherited;
  }
  const names: string[] = classNames.filter(
    (name: unknown): name is string => typeof name === 'string',
  );
  if (names.some((name: string) => COLORED_CLASSES.has(name))) {
    return names.join(' ');
  }
  return inherited;
}

/** Leere Stücke entfallen, gleiche Farbe direkt nacheinander wird zu einem Segment. */
function appendSegment(line: SyntaxSegment[], text: string, className: string | null): void {
  if (text === '') {
    return;
  }
  const lastIndex: number = line.length - 1;
  const last: SyntaxSegment | undefined = line[lastIndex];
  if (last !== undefined && last.className === className) {
    line[lastIndex] = { text: last.text + text, className };
    return;
  }
  line.push({ text, className });
}
