// Betrachter für Text, Quelltext, Log und Diff: nummerierte Zeilen, Umbruch umschaltbar; Quelltext
// mit Syntaxfarben, Log mit Filter je Ebene und am Ende aufgeschlagen, Diff mit Farben je Zeile.
(async () => {
  const style = document.createElement('style');
  style.textContent = `
    .lv-error { color: var(--diff-del-fg); background: var(--diff-del-bg); }
    .lv-warn { color: var(--warn); }
    .lv-debug { color: var(--fg-muted); }
    .lines--hide-error .lv-error, .lines--hide-warn .lv-warn,
    .lines--hide-info .lv-info, .lines--hide-debug .lv-debug { display: none; }
    .df-add { color: var(--diff-add-fg); background: var(--diff-add-bg); }
    .df-del { color: var(--diff-del-fg); background: var(--diff-del-bg); }
    .df-hunk { color: var(--code-type); background: var(--bg-bar); }
    .df-file { font-weight: 700; }
  `;
  document.head.append(style);

  let loaded;
  try {
    loaded = await fetchText();
  } catch {
    viewer.showNote('Die Datei lässt sich nicht laden.');
    return;
  }

  const kind = viewer.kind;
  const language = kind === 'code' ? languageOf(viewer.file) : '';
  const container = lines(loaded.text, classifier(kind));
  container.classList.toggle('lines--wrap', kind === 'text');
  viewer.stage.replaceChildren(...(loaded.truncated ? [capNotice()] : []), container);

  const count = container.rows.length;
  const size = viewer.formatSize(new Blob([loaded.text]).size);
  viewer.setMeta(`${count} ${count === 1 ? 'Zeile' : 'Zeilen'} · ${size}`);

  viewer.addTool(
    'Umbruch',
    (button) => {
      const wrapped = container.classList.toggle('lines--wrap');
      button.setAttribute('aria-pressed', String(wrapped));
    },
    kind === 'text',
  );
  if (kind === 'log') addLogFilters(container);
  if (kind === 'log') viewer.stage.scrollTop = viewer.stage.scrollHeight;

  if (language) await highlightLines(container, language);

  function classifier(artifactKind) {
    if (artifactKind === 'log') return logClassifier();
    if (artifactKind === 'diff') return diffClassifier();
    return undefined;
  }

  // Eine Zeile, die mit Leerraum beginnt (Stack-Trace), gehört zur Ebene der Zeile davor.
  function logClassifier() {
    let previous = 'lv-info';
    return (row) => {
      if (/^\s/.test(row)) return previous;
      if (/\b(ERROR|FATAL|CRITICAL)\b/.test(row)) previous = 'lv-error';
      else if (/\bWARN(ING)?\b/.test(row)) previous = 'lv-warn';
      else if (/\b(DEBUG|TRACE)\b/.test(row)) previous = 'lv-debug';
      else previous = 'lv-info';
      return previous;
    };
  }

  // Vor dem ersten `@@` eines Abschnitts sind `---`/`+++` Dateiköpfe, danach Zeilen.
  function diffClassifier() {
    let inHeader = true;
    return (row) => {
      if (row.startsWith('diff ')) {
        inHeader = true;
        return 'df-file';
      }
      if (row.startsWith('@@')) {
        inHeader = false;
        return 'df-hunk';
      }
      if (inHeader) return row.length > 0 ? 'df-file' : '';
      if (row.startsWith('+')) return 'df-add';
      if (row.startsWith('-')) return 'df-del';
      return '';
    };
  }

  function addLogFilters(target) {
    const levels = [
      ['error', 'Fehler'],
      ['warn', 'Warnung'],
      ['info', 'Info'],
      ['debug', 'Debug'],
    ];
    levels.forEach(([level, label]) => {
      viewer.addTool(
        label,
        (button) => {
          const hidden = target.classList.toggle(`lines--hide-${level}`);
          button.setAttribute('aria-pressed', String(!hidden));
        },
        true,
      );
    });
  }
})();
