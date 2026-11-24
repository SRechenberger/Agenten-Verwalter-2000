// Selbsttest der Syntaxfarben, vor allem der Agda-Grammatik: `node src/lib/syntax.selftest.ts` (`pnpm selftest`).
// Je Fall `PASS`/`FAIL`, Fehler (Exit 1) bei einem Fehlschlag. Node streicht die Typen; `syntax.ts` und die
// Grammatik werden relativ importiert, `lowlight` und `highlight.js` lösen sich über `node_modules` auf.
import { COLORED_CLASSES, HIGHLIGHT_LANGUAGES, highlightLines, languageOf } from './syntax.ts';

/** Die Klassen einer Zeile als Liste je Segment: `[Text, Klasse oder null]`. */
function segmentsOf(text: string): [string, string | null][] {
  return (highlightLines(text, 'agda')[0] ?? []).map((segment) => [
    segment.text,
    segment.className,
  ]);
}

/** Die Klasse, die genau der Text `word` trägt; `null`, wenn er ungefärbt ist oder in einem längeren Segment steckt. */
function classOf(text: string, word: string): string | null {
  const found = segmentsOf(text).find(([segmentText]) => segmentText === word);
  return found?.[1] ?? null;
}

/** Ob irgendein Segment des Textes die Klasse trägt. */
function hasClass(text: string, className: string): boolean {
  return segmentsOf(text).some(([, own]) => own !== null && own.split(' ').includes(className));
}

/** Ob die ganze Zeile (ohne Rand aus Leerraum) in einem Segment der Klasse steht. */
function wholeLineIs(text: string, className: string): boolean {
  const segments = segmentsOf(text).filter(([segmentText]) => segmentText.trim() !== '');
  return segments.length === 1 && segments[0]?.[1] === className;
}

interface Case {
  name: string;
  holds: () => boolean;
}

const MIXED: string = [
  '{-# OPTIONS --safe #-}',
  'module Nat.Order where',
  'data _≤_ : ℕ → ℕ → Set₁ where',
  'f = λ x → ∀ {n} → "a -- b" ?',
  "c = 'a' + 42 {! hole !} -- Rest",
].join('\n');

const CASES: Case[] = [
  { name: '.agda hat die Sprache agda', holds: () => languageOf('src/A.agda') === 'agda' },
  { name: '.ps1 hat eine Sprache (PowerShell)', holds: () => languageOf('x.ps1') !== null },
  {
    name: '.lagda.md behält die Sprache der äußeren Endung',
    holds: () => languageOf('A.lagda.md') === 'md',
  },
  {
    name: 'Die Sprachliste enthält agda und powershell',
    holds: () => 'agda' in HIGHLIGHT_LANGUAGES && 'powershell' in HIGHLIGHT_LANGUAGES,
  },
  {
    name: 'Die Sprachliste enthält weiter die Sprachen von common',
    holds: () => 'typescript' in HIGHLIGHT_LANGUAGES && 'markdown' in HIGHLIGHT_LANGUAGES,
  },
  {
    name: 'map-with ist ein Bezeichner, kein Schlüsselwort',
    holds: () => !hasClass('map-with f xs = x', 'hljs-keyword'),
  },
  {
    name: '≤-trans, _∷_, +-comm und Dec? sind keine Schlüsselwörter und kein Loch',
    holds: () =>
      !hasClass('≤-trans _∷_ +-comm Dec?', 'hljs-keyword') &&
      !hasClass('≤-trans _∷_ +-comm Dec?', 'hljs-meta'),
  },
  {
    name: 'with allein ist ein Schlüsselwort',
    holds: () => classOf('f with x', 'with') === 'hljs-keyword',
  },
  {
    name: 'module, where, data und open import sind Schlüsselwörter',
    holds: () =>
      classOf('module A where', 'module') === 'hljs-keyword' &&
      classOf('module A where', 'where') === 'hljs-keyword' &&
      classOf('data D', 'data') === 'hljs-keyword' &&
      classOf('open import A', 'import') === 'hljs-keyword',
  },
  {
    name: '∀, → und λ sind Schlüsselwörter',
    holds: () =>
      classOf('f = λ x → ∀ y', 'λ') === 'hljs-keyword' &&
      classOf('f = λ x → ∀ y', '→') === 'hljs-keyword' &&
      classOf('f = λ x → ∀ y', '∀') === 'hljs-keyword',
  },
  {
    name: 'Set₁, Setω und Prop sind Typen',
    holds: () =>
      classOf('A : Set₁', 'Set₁') === 'hljs-type' &&
      classOf('A : Setω', 'Setω') === 'hljs-type' &&
      classOf('A : Prop', 'Prop') === 'hljs-type' &&
      classOf('A : Set', 'Set') === 'hljs-type',
  },
  {
    name: 'Ein Pragma ist hljs-meta',
    holds: () => wholeLineIs('{-# OPTIONS --safe #-}', 'hljs-meta'),
  },
  {
    name: 'Ein Pragma direkt nach einem Blockkommentar ist weiter hljs-meta',
    holds: () => classOf('{- c -} {-# NOINLINE f #-}', '{-# NOINLINE f #-}') === 'hljs-meta',
  },
  {
    name: 'Ein Loch {! !} ist hljs-meta',
    holds: () => wholeLineIs('{! !}', 'hljs-meta') && hasClass('f = {! x !}', 'hljs-meta'),
  },
  {
    name: 'Ein allein stehendes ? ist hljs-meta',
    holds: () => classOf('f = ?', '?') === 'hljs-meta' && classOf('f (?)', '?') === 'hljs-meta',
  },
  {
    name: '? in _?_ und Dec? ist kein Loch',
    holds: () => !hasClass('_?_', 'hljs-meta') && !hasClass('Dec?', 'hljs-meta'),
  },
  {
    name: 'Ein Zeilenkommentar -- ist ganz hljs-comment',
    holds: () => wholeLineIs('-- x', 'hljs-comment'),
  },
  {
    name: 'Ein Zeilenkommentar ohne Leerraum nach -- gilt, --> ist ein Bezeichner',
    holds: () => wholeLineIs('--text', 'hljs-comment') && !hasClass('a --> b', 'hljs-comment'),
  },
  {
    name: 'Ein geschachtelter Blockkommentar ist ganz hljs-comment',
    holds: () => wholeLineIs('{- a {- b -} c -}', 'hljs-comment'),
  },
  {
    name: '-- in einer Zeichenkette ist kein Kommentar',
    holds: () =>
      classOf('s = "a -- b"', '"a -- b"') === 'hljs-string' &&
      !hasClass('s = "a -- b"', 'hljs-comment'),
  },
  {
    name: 'Zeichenketten, Zeichen und Zahlen werden gefärbt',
    holds: () =>
      hasClass('s = "x\\"y"', 'hljs-string') &&
      classOf("c = 'a'", "'a'") === 'hljs-string' &&
      classOf('n = 42', '42') === 'hljs-number',
  },
  {
    name: "Ein Strich im Bezeichner x' y' ist kein Zeichen",
    holds: () => !hasClass("x' = y'", 'hljs-string'),
  },
  {
    name: 'Jede gelieferte Klasse steht in COLORED_CLASSES',
    holds: () =>
      highlightLines(MIXED, 'agda')
        .flat()
        .every(
          ({ className }) =>
            className === null || className.split(' ').every((name) => COLORED_CLASSES.has(name)),
        ),
  },
];

let failed = 0;
for (const { name, holds } of CASES) {
  let ok = false;
  try {
    ok = holds();
  } catch (error) {
    console.log(`  ${String(error)}`);
  }
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}`);
  if (!ok) {
    failed += 1;
  }
}
if (failed > 0) {
  throw new Error(`${String(failed)} von ${String(CASES.length)} Fällen gescheitert`);
}
