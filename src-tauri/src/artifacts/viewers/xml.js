// Betrachter für XML: Baum der Elemente mit Attributen (DOMParser) oder Quelltext mit Syntaxfarben.
// Kaputtes XML zeigt den Quelltext mit der Fehlerzeile.
const TEXT_NODE = 3;
const CDATA_NODE = 4;
const ELEMENT_NODE = 1;

(async () => {
  let loaded;
  try {
    loaded = await fetchText();
  } catch {
    viewer.showNote('Die Datei lässt sich nicht laden.');
    return;
  }

  const size = viewer.formatSize(new Blob([loaded.text]).size);
  const parsed = new DOMParser().parseFromString(loaded.text, 'application/xml');
  const parseError = parsed.querySelector('parsererror');
  if (parseError || loaded.truncated) {
    showInvalid(parseError ? parseError.textContent : 'Datei zu groß für den Baum, hier gekürzt');
    return;
  }

  const root = nodeFromElement(parsed.documentElement, `/${parsed.documentElement.nodeName}`);
  viewer.setMeta(`${size} · ${countNodes(root)} Knoten`);
  const tree = createTreeView([{ title: '', node: root, prefix: '' }]);
  let source;
  addViewSwitch([
    {
      label: 'Baum',
      show: () => {
        tree.searchInput.hidden = false;
        viewer.stage.replaceChildren(tree);
      },
    },
    {
      label: 'Quelltext',
      show: () => {
        tree.searchInput.hidden = true;
        if (!source) {
          source = lines(loaded.text);
          viewer.stage.replaceChildren(source);
          highlightLines(source, 'xml');
          return;
        }
        viewer.stage.replaceChildren(source);
      },
    },
  ]);

  function attributesOf(element) {
    return Array.from(element.attributes)
      .map((attribute) => ` ${attribute.name}="${attribute.value}"`)
      .join('');
  }

  function textOf(child) {
    if (child.nodeType !== TEXT_NODE && child.nodeType !== CDATA_NODE) return '';
    return child.nodeValue.trim();
  }

  // Ein Element nur mit Text ist ein Blatt; sonst ein Container, dessen Text als Kind `#text` erscheint.
  function nodeFromElement(element, path) {
    const extra = attributesOf(element);
    const childElements = Array.from(element.children);
    const base = { label: element.nodeName, path, extra, isIndexed: false };
    if (childElements.length === 0) {
      const text = Array.from(element.childNodes).map(textOf).filter((part) => part !== '');
      return { ...base, type: 'string', display: text.join(' ') };
    }
    const counts = new Map();
    childElements.forEach((child) => counts.set(child.nodeName, (counts.get(child.nodeName) ?? 0) + 1));
    const seen = new Map();
    let textCount = 0;
    const children = [];
    // In Dokumentreihenfolge, damit Text zwischen Elementen (gemischter Inhalt) an seiner Stelle bleibt.
    element.childNodes.forEach((child) => {
      if (child.nodeType === ELEMENT_NODE) {
        const position = (seen.get(child.nodeName) ?? 0) + 1;
        seen.set(child.nodeName, position);
        const step = counts.get(child.nodeName) > 1 ? `${child.nodeName}[${position}]` : child.nodeName;
        children.push(nodeFromElement(child, `${path}/${step}`));
        return;
      }
      const text = textOf(child);
      if (text === '') return;
      textCount += 1;
      children.push({ label: '#text', path: `${path}/text()[${textCount}]`, type: 'string', display: text, isIndexed: true });
    });
    return { ...base, type: 'map', summary: `< ${childElements.length} >`, children };
  }

  function showInvalid(message) {
    const container = lines(loaded.text);
    viewer.stage.replaceChildren(...(loaded.truncated ? [capNotice()] : []), container);
    const errorLine = Number(/line (?:number )?(\d+)/i.exec(message)?.[1] ?? 0);
    if (errorLine) markErrorLine(container, errorLine);
    const detail = /(error on line[^]*?)(?:Below is|$)/.exec(message)?.[1] ?? message.trim().split('\n')[0];
    viewer.setMeta(`Kein gültiges XML${errorLine ? ` (Zeile ${errorLine})` : ''}: ${detail.trim()}`);
    highlightLines(container, 'xml');
  }
})();
