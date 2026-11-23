// Zeilen-Diff für den Kasten eines Vorschlags (ADR 075, „Per MCP“, M6): längste gemeinsame Teilfolge über
// Zeilen, untereinander wie die Changes-Ansicht. Rein und ohne Abhängigkeit; der Selbsttest läuft mit `node`.

export type DiffLine = { kind: 'same' | 'removed' | 'added'; text: string };

/** Zeilen an `\n`; ein leerer Text hat keine Zeile, ein `\n` am Ende eine leere letzte Zeile. */
function linesOf(text: string): string[] {
  return text === '' ? [] : text.split('\n');
}

/** `before` → `after` Zeile für Zeile; in einem geänderten Stück stehen die entfernten Zeilen vor den neuen. */
export function lineDiff(before: string, after: string): DiffLine[] {
  const a: string[] = linesOf(before);
  const b: string[] = linesOf(after);
  // Gleicher Anfang und gleiches Ende kosten keine Tabelle.
  let head = 0;
  while (head < a.length && head < b.length && a[head] === b[head]) {
    head += 1;
  }
  let tail = 0;
  while (
    tail < a.length - head &&
    tail < b.length - head &&
    a[a.length - 1 - tail] === b[b.length - 1 - tail]
  ) {
    tail += 1;
  }
  const middleA: string[] = a.slice(head, a.length - tail);
  const middleB: string[] = b.slice(head, b.length - tail);
  const same = (text: string): DiffLine => ({ kind: 'same', text });
  return [
    ...a.slice(0, head).map(same),
    ...middle(middleA, middleB),
    ...a.slice(a.length - tail).map(same),
  ];
}

// ponytail: volle Tabelle (n × m) für das geänderte Mittelstück; eine Vorlage hat höchstens 16 000 Zeichen, also
// selten mehr als einige hundert Zeilen. Myers erst, wenn Vorschläge mit tausenden geänderten Zeilen spürbar hängen.
function middle(a: readonly string[], b: readonly string[]): DiffLine[] {
  const width: number = b.length + 1;
  // `common[i * width + j]`: Länge der gemeinsamen Teilfolge von `a[i..]` und `b[j..]`.
  const common = new Uint32Array((a.length + 1) * width);
  for (let i = a.length - 1; i >= 0; i -= 1) {
    for (let j = b.length - 1; j >= 0; j -= 1) {
      common[i * width + j] =
        a[i] === b[j]
          ? (common[(i + 1) * width + j + 1] ?? 0) + 1
          : Math.max(common[(i + 1) * width + j] ?? 0, common[i * width + j + 1] ?? 0);
    }
  }
  const lines: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    const lineA: string = a[i] ?? '';
    const lineB: string = b[j] ?? '';
    if (lineA === lineB) {
      lines.push({ kind: 'same', text: lineA });
      i += 1;
      j += 1;
    } else if ((common[(i + 1) * width + j] ?? 0) >= (common[i * width + j + 1] ?? 0)) {
      lines.push({ kind: 'removed', text: lineA });
      i += 1;
    } else {
      lines.push({ kind: 'added', text: lineB });
      j += 1;
    }
  }
  for (; i < a.length; i += 1) {
    lines.push({ kind: 'removed', text: a[i] ?? '' });
  }
  for (; j < b.length; j += 1) {
    lines.push({ kind: 'added', text: b[j] ?? '' });
  }
  return lines;
}
