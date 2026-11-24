import type { HLJSApi, Language, LanguageFn } from 'highlight.js';

// Rein lexikalische Grammatik für Agda; highlight.js bringt keine mit. Aufbau nach `haskell.js`.
//
// - Bezeichner enden nur an Leerraum und an `( ) { } ; . @ "` (`$pattern`). So bleiben `map-with`,
//   `≤-trans`, `_∷_`, `+-comm` und `Dec?` je ein Wort, und `with` in `map-with` ist kein Schlüsselwort.
// - Ein `?` ist nur dann ein Loch, wenn es als ganzes Wort steht (`meta`); in `_?_` und `Dec?` nicht.
// - Pragmas `{-# … #-}` stehen vor dem Blockkommentar `{- … -}`, sonst fräße dieser sie.
// - Der Zeilenkommentar `--` gilt auch ohne Leerraum danach (`--text`), nur nicht vor einem Operatorzeichen:
//   `-->` bleibt ein Bezeichner. Mitten in einem Wort (`a--b`) beginnt er ebenfalls (ponytail: ohne Lookbehind
//   nicht zu trennen).
// - `Set` und `Prop`, auch mit Stufe (`Set₁`, `Setω`), sind Typen.
// - Zahlen und Zeichen sind einfach gehalten: `x-2` färbt die 2, `ℕ'a'` das Zeichen (ponytail: erst
//   mit einem Wortrand-Modus ändern, falls das stört).
// - Kein Flag `u` nötig: alle benutzten Zeichen liegen in der BMP.
const KEYWORDS: string =
  'data record field constructor where module open import using hiding renaming to public let in with ' +
  'rewrite postulate abstract private instance mutual infix infixl infixr pattern syntax variable macro ' +
  'primitive forall λ ∀ → do opaque unfolding coinductive inductive eta-equality no-eta-equality ' +
  'overlap interleaved -> quote quoteTerm tactic unquote unquoteDecl unquoteDef';

const agda: LanguageFn = (hljs: HLJSApi): Language => {
  const BLOCK_COMMENT = hljs.COMMENT(/\{-/, /-\}/, { contains: ['self'] });
  return {
    name: 'Agda',
    keywords: {
      $pattern: '[^\\s(){};.@"]+', // als Zeichenkette: die Typen von highlight.js erlauben hier keinen RegExp
      keyword: KEYWORDS,
      type: 'Set Prop',
      meta: '?',
    },
    contains: [
      { className: 'meta', begin: /\{-#/, end: /#-\}/ },
      BLOCK_COMMENT,
      hljs.COMMENT(/--+(?![!#$%&*+/<=>?^~|\\:])/, '$'),
      { className: 'meta', begin: /\{!/, end: /!\}/ },
      { className: 'type', begin: /\b(?:Set|Prop)[₀-₉ω]+/, relevance: 0 },
      hljs.QUOTE_STRING_MODE,
      { className: 'string', begin: /\B'(?:\\.|[^\\'\n])'/, relevance: 0 },
      { className: 'number', begin: /\b[0-9]+(?![^\s(){};.@"])/, relevance: 0 },
    ],
  };
};

export default agda;
