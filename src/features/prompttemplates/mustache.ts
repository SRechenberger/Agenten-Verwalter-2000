// Die Mustache-Teilmenge der Prompt-Vorlagen (ADR 075, E1 bis E3, A3 bis A6): Felder aus Vorlagentext und
// Namensvorlage, Prüfung der Syntax und Füllen, als reine Funktionen ohne Abhängigkeit. Der Core prüft
// dieselbe Syntax in `src-tauri/src/prompt_templates/syntax.rs`; beide gehen die Fallliste
// `syntax_cases.json` durch und müssen jeden Fall gleich beantworten: dieselbe Reihenfolge der Prüfungen,
// dieselben Sätze, Zeile und Spalte in Codepunkten, 1-basiert. Der Selbsttest läuft mit `node`: Werte
// kommen hier nicht über `@/`.

export type TemplateField = { kind: 'text' | 'flag'; name: string; help: string | null };
export type TemplateError = {
  source: 'text' | 'nameTemplate';
  message: string;
  line: number;
  column: number;
};
export type Parsed = { ok: true; fields: TemplateField[] } | { ok: false; error: TemplateError };

const MAX_TAG_NAME_CHARS = 40;

/** Ein Tag mit seiner Stelle im Text (UTF-16-Indizes, `end` hinter `}}`). */
type Tag =
  | { kind: 'comment'; text: string; start: number; end: number }
  | { kind: 'variable'; name: string; start: number; end: number }
  | { kind: 'open'; name: string; inverted: boolean; start: number; end: number }
  | { kind: 'close'; name: string; start: number; end: number };

type Scanned = { ok: true; tags: Tag[] } | { ok: false; error: TemplateError };

/** Ein Name ist entweder Variable oder Abschnitt, in Text und Namensvorlage zusammen (A3, A4). */
type Kinds = Map<string, 'variable' | 'section'>;

/** Nur ASCII-Leerraum, wie `trim_tag` im Core (`String.prototype.trim` nähme auch U+FEFF). */
function trimTag(content: string): string {
  return content.replace(/^[ \t\n\r\f]+|[ \t\n\r\f]+$/g, '');
}

function codePoints(text: string): number {
  return Array.from(text).length;
}

function nameProblem(name: string): string | null {
  if (name === '') {
    return 'Im Tag fehlt der Name.';
  }
  if (name.includes('{') || name.includes('}')) {
    return `Der Name „${name}“ enthält „{“ oder „}“.`;
  }
  if (codePoints(name) > MAX_TAG_NAME_CHARS) {
    return `Der Name „${name}“ ist länger als 40 Zeichen.`;
  }
  return null;
}

function errorAt(
  source: string,
  which: TemplateError['source'],
  at: number,
  message: string,
): TemplateError {
  const before: string = source.slice(0, at);
  const lineStart: number = before.lastIndexOf('\n') + 1;
  return {
    source: which,
    message,
    line: before.split('\n').length,
    column: codePoints(before.slice(lineStart)) + 1,
  };
}

/** Liest die Tags in Lesereihenfolge und meldet den ersten Fehler; dieselben Schritte wie `check_source`. */
function scan(source: string, which: TemplateError['source'], kinds: Kinds): Scanned {
  const fail = (at: number, message: string): Scanned => ({
    ok: false,
    error: errorAt(source, which, at, message),
  });
  const tags: Tag[] = [];
  const open: { name: string; start: number }[] = [];
  let rest = 0;
  for (;;) {
    const start: number = source.indexOf('{{', rest);
    if (start < 0) {
      break;
    }
    const triple: boolean = source.startsWith('{', start + 2);
    const inner: number = start + (triple ? 3 : 2);
    const closing: string = triple ? '}}}' : '}}';
    const length: number = source.indexOf(closing, inner) - inner;
    if (length < 0) {
      return fail(
        start,
        triple ? '„{{{“ ohne schließendes „}}}“.' : '„{{“ ohne schließendes „}}“.',
      );
    }
    const end: number = inner + length + closing.length;
    rest = end;
    const content: string = trimTag(source.slice(inner, inner + length));
    // `{{{name}}}` ist immer eine Variable (E1).
    const sigil: string = triple ? '' : content.charAt(0);
    if (sigil === '!') {
      tags.push({ kind: 'comment', text: trimTag(content.slice(1)), start, end });
      continue;
    }
    if (sigil === '>' || sigil === '=' || sigil === '&') {
      return fail(start, `„{{${sigil}“ wird nicht unterstützt.`);
    }
    if (sigil === '#' || sigil === '^' || sigil === '/') {
      const name: string = trimTag(content.slice(1));
      const problem: string | null = nameProblem(name);
      if (problem !== null) {
        return fail(start, problem);
      }
      if (sigil === '/') {
        const opened = open.pop();
        if (opened === undefined) {
          return fail(start, `„{{/${name}}}“ schließt keinen offenen Abschnitt.`);
        }
        if (opened.name !== name) {
          return fail(start, `„{{/${name}}}“ passt nicht zum offenen Abschnitt „${opened.name}“.`);
        }
        tags.push({ kind: 'close', name, start, end });
        continue;
      }
      if (kinds.get(name) === 'variable') {
        return fail(start, `„${name}“ ist schon eine Variable und kann kein Abschnitt sein.`);
      }
      kinds.set(name, 'section');
      open.push({ name, start });
      tags.push({ kind: 'open', name, inverted: sigil === '^', start, end });
      continue;
    }
    const problem: string | null = nameProblem(content);
    if (problem !== null) {
      return fail(start, problem);
    }
    if (kinds.get(content) === 'section') {
      return fail(start, `„${content}“ ist schon ein Abschnitt und kann keine Variable sein.`);
    }
    kinds.set(content, 'variable');
    tags.push({ kind: 'variable', name: content, start, end });
  }
  const last: { name: string; start: number } | undefined = open[open.length - 1];
  if (last !== undefined) {
    return fail(last.start, `Der Abschnitt „${last.name}“ ist nicht geschlossen.`);
  }
  return { ok: true, tags };
}

/** Die Felder beider Texte, zuerst die des Vorlagentexts (A3), je in der Reihenfolge des ersten Auftretens
 *  (E2). Ein Kommentar wird Hilfetext der nächsten Variablen seines Texts, wenn sie noch keinen hat (A6). */
export function parseTemplate(text: string, nameTemplate: string): Parsed {
  const kinds: Kinds = new Map();
  const fields: TemplateField[] = [];
  const sources: [string, TemplateError['source']][] = [
    [text, 'text'],
    [nameTemplate, 'nameTemplate'],
  ];
  for (const [source, which] of sources) {
    const scanned: Scanned = scan(source, which, kinds);
    if (!scanned.ok) {
      return scanned;
    }
    let help: string | null = null;
    for (const tag of scanned.tags) {
      if (tag.kind === 'comment') {
        help = tag.text === '' ? null : tag.text;
        continue;
      }
      if (tag.kind === 'close') {
        continue;
      }
      let field: TemplateField | undefined = fields.find(
        (known: TemplateField) => known.name === tag.name,
      );
      if (field === undefined) {
        field = { kind: tag.kind === 'variable' ? 'text' : 'flag', name: tag.name, help: null };
        fields.push(field);
      }
      if (tag.kind === 'variable') {
        if (field.help === null) {
          field.help = help;
        }
        help = null;
      }
    }
  }
  return { ok: true, fields };
}

/** Wie weit ein Tag den Text verbraucht: allein auf seiner Zeile nimmt ein Abschnitts-, Umkehr-, Schluss- oder
 *  Kommentar-Tag die ganze Zeile samt Zeilenende mit (A5); sonst nur sich selbst. */
function consumed(source: string, tag: Tag): { start: number; end: number } {
  if (tag.kind === 'variable') {
    return tag;
  }
  const lineStart: number = source.lastIndexOf('\n', tag.start - 1) + 1;
  const newline: number = source.indexOf('\n', tag.end);
  const lineEnd: number = newline < 0 ? source.length : newline;
  const isAlone: boolean =
    /^[ \t]*$/.test(source.slice(lineStart, tag.start)) &&
    /^[ \t]*\r?$/.test(source.slice(tag.end, lineEnd));
  if (!isAlone) {
    return tag;
  }
  return { start: lineStart, end: newline < 0 ? source.length : newline + 1 };
}

/** Füllt einen der beiden Texte. Ein leeres oder fehlendes Textfeld bleibt `{{name}}` (E3), ein Abschnitt
 *  steht nur mit Haken, eine Umkehrung nur ohne; kein Escaping, Kommentare entfallen. Mit Syntaxfehler kommt
 *  der Text unverändert zurück: Ein Formular gibt es nur zu einer gültigen Vorlage. */
export function renderTemplate(source: string, values: Record<string, string | boolean>): string {
  const scanned: Scanned = scan(source, 'text', new Map());
  if (!scanned.ok) {
    return source;
  }
  let output = '';
  let position = 0;
  // Je offenem Abschnitt, ob er seinen Inhalt verbirgt; Text kommt nur in die Ausgabe, wenn keiner verbirgt.
  const hiding: boolean[] = [];
  const isShown = (): boolean => !hiding.includes(true);
  for (const tag of scanned.tags) {
    const range: { start: number; end: number } = consumed(source, tag);
    if (isShown()) {
      output += source.slice(position, range.start);
    }
    position = range.end;
    if (tag.kind === 'open') {
      const isOn: boolean = values[tag.name] === true;
      hiding.push(tag.inverted ? isOn : !isOn);
    } else if (tag.kind === 'close') {
      hiding.pop();
    } else if (tag.kind === 'variable' && isShown()) {
      const value: string | boolean | undefined = values[tag.name];
      output += typeof value === 'string' && value !== '' ? value : `{{${tag.name}}}`;
    }
  }
  return output + source.slice(position);
}
