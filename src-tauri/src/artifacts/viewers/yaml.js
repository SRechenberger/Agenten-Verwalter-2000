// Betrachter für YAML: Quelltext mit Syntaxfarben und Einklappen nach Einrückung, dazu ein Baum
// (js-yaml); mehrere Dokumente stehen als Abschnitte untereinander. Ohne Bibliothek bleibt der Quelltext.
(async () => {
  const style = document.createElement('style');
  style.textContent = `
    .line .fold { position: absolute; left: calc(var(--gutter) - 12px); width: 10px; color: var(--fg-muted); cursor: pointer; user-select: none; }
    .line--folded::after { content: ' …'; color: var(--fg-muted); }
    .line[hidden] { display: none; }
  `;
  document.head.append(style);

  let loaded;
  try {
    loaded = await fetchText();
  } catch {
    viewer.showNote('Die Datei lässt sich nicht laden.');
    return;
  }

  const source = lines(loaded.text);
  const notices = loaded.truncated ? [capNotice()] : [];
  viewer.stage.replaceChildren(...notices, source);
  const size = viewer.formatSize(new Blob([loaded.text]).size);
  viewer.setMeta(`${source.rows.length} Zeilen · ${size}`);

  const documents = await parseDocuments(loaded);
  await highlightLines(source, 'yaml');
  addFolds(source);

  if (documents) {
    addViewSwitch(
      [
        { label: 'Quelltext', show: () => viewer.stage.replaceChildren(...notices, source) },
        { label: 'Baum', show: () => showTree(documents) },
      ],
      0,
    );
  }

  /** Die Dokumente der Datei, oder `null`, wenn die Bibliothek fehlt oder der Text kein YAML ist. */
  async function parseDocuments(file) {
    try {
      await loadLibrary('yaml');
    } catch {
      viewer.setMeta('Bibliothek nicht erreichbar — Quelltext ohne Baum');
      return null;
    }
    try {
      return window.jsyaml.loadAll(file.text, null, { schema: window.jsyaml.CORE_SCHEMA });
    } catch (error) {
      viewer.setMeta(`Kein gültiges YAML: ${error.message.split('\n')[0]}`);
      return null;
    }
  }

  let treeView;
  function showTree(parsed) {
    if (!treeView) {
      treeView = createTreeView(
        parsed.map((value, index) => ({
          title: `Dokument ${index + 1}`,
          node: nodeFromValue(value, '$', '$'),
          prefix: `d${index + 1}`,
        })),
      );
    }
    treeView.searchInput.hidden = false;
    viewer.stage.replaceChildren(...notices, treeView);
  }

  // Eine Zeile lässt sich einklappen, wenn die nächste nicht leere Zeile tiefer eingerückt ist; eingeklappt
  // werden alle folgenden Zeilen, die tiefer eingerückt (oder leer) sind.
  function addFolds(container) {
    const elements = Array.from(container.children);
    const indents = container.rows.map((row) => (row.trim() === '' ? -1 : /^\s*/.exec(row)[0].length));
    const foldEnds = new Map();
    indents.forEach((indent, index) => {
      if (indent < 0) return;
      let end = index;
      for (let next = index + 1; next < indents.length; next += 1) {
        if (indents[next] >= 0 && indents[next] <= indent) break;
        end = next;
      }
      while (end > index && indents[end] < 0) end -= 1;
      if (end > index) foldEnds.set(index, end);
    });
    const folded = new Set();

    const refresh = () => {
      let hiddenUntil = -1;
      elements.forEach((element, index) => {
        element.hidden = index <= hiddenUntil;
        if (element.hidden) return;
        element.classList.toggle('line--folded', folded.has(index));
        if (folded.has(index)) hiddenUntil = foldEnds.get(index);
      });
      foldEnds.forEach((end, index) => {
        const marker = elements[index].querySelector('.fold');
        if (marker) marker.textContent = folded.has(index) ? '▸' : '▾';
      });
    };

    foldEnds.forEach((end, index) => {
      const marker = document.createElement('span');
      marker.className = 'fold';
      marker.textContent = '▾';
      marker.setAttribute('role', 'button');
      marker.setAttribute('aria-label', 'Einklappen');
      marker.addEventListener('click', (event) => {
        event.stopPropagation();
        if (folded.has(index)) folded.delete(index);
        else folded.add(index);
        refresh();
      });
      elements[index].prepend(marker);
    });
  }
})();
