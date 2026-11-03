// Baumansicht für JSON, YAML, XML und einzelne JSON-Lines-Datensätze (ADR 047). Ein Baum besteht
// aus Knoten `{ label, path, type, display?, extra?, summary?, children? }`; `nodeFromValue` baut
// sie aus einem JavaScript-Wert, der XML-Betrachter baut sie selbst. Jede Zeile ist ein Kommentar-Anker.
const MAX_CHILDREN_SHOWN = 1000;
const MAX_SCALAR_CHARS = 400;
const AUTO_OPEN_DEPTH = 2;
const PLAIN_KEY = /^[A-Za-z_][A-Za-z0-9_]*$/;

const treeStyle = document.createElement('style');
treeStyle.textContent = `
  .tree { padding: 8px 12px; font: 12px/1.6 var(--font-mono); }
  .tree-section + .tree-section { margin-top: 16px; }
  .tree-title { margin: 0 0 4px; color: var(--fg-muted); font: 600 11px/1.4 var(--font-sans); text-transform: uppercase; }
  .tn__children { margin-left: 8px; padding-left: 12px; border-left: 1px solid var(--border); }
  .tn__row { display: flex; align-items: baseline; gap: 6px; min-height: 1.6em; white-space: pre-wrap; overflow-wrap: anywhere; }
  .tn__row:hover { background: var(--bg-bar); }
  .tn__toggle, .tn__spacer { flex: none; width: 14px; padding: 0; border: 0; background: none; color: var(--fg-muted); font: inherit; text-align: center; }
  .tn__toggle { cursor: pointer; }
  .tn__key { color: var(--accent); }
  .tn__key--index { color: var(--fg-muted); }
  .tn__value--string { color: var(--code-string); }
  .tn__value--number, .tn__value--boolean { color: var(--warn); }
  .tn__value--null, .tn__summary, .tn__attr { color: var(--fg-muted); }
  .tn__hit { background: color-mix(in srgb, var(--accent) 25%, transparent); }
  .tn__copy, .tn__more { flex: none; padding: 0 6px; border: 1px solid var(--border); border-radius: 3px; background: var(--bg); color: var(--fg-muted); font: 11px/16px var(--font-sans); cursor: pointer; }
  .tn__copy { margin-left: auto; visibility: hidden; }
  .tn__row:hover .tn__copy, .tn__copy:focus-visible { visibility: visible; }
  .tn__path { margin-left: auto; padding: 0 6px; border: 1px solid var(--accent); border-radius: 3px; background: var(--bg); color: var(--fg); font: inherit; }
`;
document.head.append(treeStyle);

/** Pfad-Schreibweise nach JSONPath: `$.typen[0].endung`, Schlüssel mit Sonderzeichen als `['…']`. */
function jsonPathKey(parentPath, key) {
  if (PLAIN_KEY.test(key)) return `${parentPath}.${key}`;
  return `${parentPath}['${key.replace(/\\/g, '\\\\').replace(/'/g, "\\'")}']`;
}

function nodeFromValue(value, label, path, isIndexed = false) {
  const base = { label, path, isIndexed };
  if (Array.isArray(value)) {
    const children = value.map((item, index) =>
      nodeFromValue(item, String(index), `${path}[${index}]`, true),
    );
    return { ...base, type: 'list', summary: `[ ${value.length} ]`, children };
  }
  if (value !== null && typeof value === 'object') {
    const keys = Object.keys(value);
    const children = keys.map((key) => nodeFromValue(value[key], key, jsonPathKey(path, key)));
    return { ...base, type: 'map', summary: `{ ${keys.length} }`, children };
  }
  if (value === null) return { ...base, type: 'null', display: 'null' };
  if (typeof value === 'string') return { ...base, type: 'string', display: JSON.stringify(value) };
  if (typeof value === 'number' || typeof value === 'boolean') {
    return { ...base, type: typeof value, display: String(value) };
  }
  return { ...base, type: 'string', display: String(value) };
}

function countNodes(node) {
  let total = 1;
  for (const child of node.children ?? []) total += countNodes(child);
  return total;
}

/** Zeilen-ID, die als CSS-Selektor taugt (die Brücke findet Kommentare per `#id` wieder). */
function nodeId(prefix, path) {
  const safe = path.replace(/[^A-Za-z0-9-]/g, (char) => `_${char.charCodeAt(0).toString(16)}_`);
  return `n${prefix}-${safe}`;
}

function nodeSearchText(node) {
  return `${node.label} ${node.display ?? ''} ${node.extra ?? ''}`.toLowerCase();
}

function span(className, text) {
  const element = document.createElement('span');
  element.className = className;
  element.textContent = text;
  return element;
}

/** `execCommand` geht im abgeschotteten Rahmen ohne Freigabe, solange der Klick es auslöst. */
function copyByCommand(text) {
  const field = document.createElement('textarea');
  field.value = text;
  field.style.position = 'fixed';
  field.style.opacity = '0';
  document.body.append(field);
  field.select();
  let copied = false;
  try {
    copied = document.execCommand('copy');
  } catch {
    copied = false;
  }
  field.remove();
  return copied;
}

function copyByClipboard(text) {
  const timeout = new Promise((resolve) => setTimeout(() => resolve(false), 1000));
  const write = navigator.clipboard.writeText(text).then(
    () => true,
    () => false,
  );
  return Promise.race([write, timeout]);
}

/** Legt den Pfad in die Zwischenablage; geht beides nicht, zeigt die Zeile ihn zum Markieren. */
async function copyPath(path, button) {
  const copied = copyByCommand(path) || (await copyByClipboard(path));
  if (copied) {
    button.textContent = 'Kopiert';
    setTimeout(() => {
      button.textContent = 'Pfad kopieren';
    }, 1200);
    return;
  }
  const input = document.createElement('input');
  input.className = 'tn__path';
  input.readOnly = true;
  input.value = path;
  input.size = Math.max(10, path.length);
  button.replaceWith(input);
  input.focus();
  input.select();
}

/** Ein aufklappbarer Baum; `prefix` hält die Zeilen-IDs mehrerer Bäume auf einer Seite auseinander. */
function createTree(root, prefix = '') {
  const element = document.createElement('div');
  element.className = 'tree';
  const openOverrides = new Map();
  const listsShownFully = new Set();
  let matches = null;
  let query = '';

  function markMatches(node, parentMatched, results) {
    const selfMatches = nodeSearchText(node).includes(query);
    let below = false;
    for (const child of node.children ?? []) {
      if (markMatches(child, parentMatched || selfMatches, results)) below = true;
    }
    results.set(node, { selfMatches, below, visible: parentMatched || selfMatches || below });
    return selfMatches || below;
  }

  function isOpen(node, depth) {
    if (openOverrides.has(node.path)) return openOverrides.get(node.path);
    if (matches) return matches.get(node).below;
    return depth < AUTO_OPEN_DEPTH;
  }

  function renderRow(node, depth, open) {
    const row = document.createElement('div');
    row.className = 'tn__row';
    row.id = nodeId(prefix, node.path);
    row.dataset.path = node.path;
    row.setAttribute('data-anchor', '');
    const isContainer = Boolean(node.children);
    if (isContainer) {
      const toggle = document.createElement('button');
      toggle.type = 'button';
      toggle.className = 'tn__toggle';
      toggle.textContent = open ? '▾' : '▸';
      toggle.setAttribute('aria-expanded', String(open));
      toggle.setAttribute('aria-label', `${node.label} ${open ? 'zuklappen' : 'aufklappen'}`);
      toggle.addEventListener('click', () => {
        openOverrides.set(node.path, !open);
        redrawNode(row.parentElement, node, depth);
      });
      row.append(toggle);
    } else {
      row.append(span('tn__spacer', ''));
    }
    const hit = matches?.get(node).selfMatches ? ' tn__hit' : '';
    row.append(span(`tn__key${node.isIndexed ? ' tn__key--index' : ''}${hit}`, node.label));
    row.append(span('tn__summary', ':'));
    if (isContainer) {
      if (node.summary) row.append(span('tn__summary', node.summary));
    } else {
      const text =
        node.display.length > MAX_SCALAR_CHARS
          ? `${node.display.slice(0, MAX_SCALAR_CHARS)}…`
          : node.display;
      row.append(span(`tn__value--${node.type}`, text));
    }
    if (node.extra) row.append(span('tn__attr', node.extra));
    const copy = document.createElement('button');
    copy.type = 'button';
    copy.className = 'tn__copy';
    copy.textContent = 'Pfad kopieren';
    copy.addEventListener('click', () => copyPath(node.path, copy));
    row.append(copy);
    return row;
  }

  function renderChildren(node, depth) {
    const children = document.createElement('div');
    children.className = 'tn__children';
    const visible = node.children.filter((child) => !matches || matches.get(child).visible);
    const limit = listsShownFully.has(node.path) ? visible.length : MAX_CHILDREN_SHOWN;
    for (const child of visible.slice(0, limit)) {
      children.append(renderNode(child, depth + 1));
    }
    if (visible.length > limit) {
      const more = document.createElement('button');
      more.type = 'button';
      more.className = 'tn__more';
      more.textContent = `… ${visible.length - limit} weitere anzeigen`;
      more.addEventListener('click', () => {
        listsShownFully.add(node.path);
        redrawNode(children.parentElement, node, depth);
      });
      children.append(more);
    }
    return children;
  }

  function renderNode(node, depth) {
    if (matches && !matches.get(node).visible) return document.createDocumentFragment();
    const wrapper = document.createElement('div');
    wrapper.className = 'tn';
    const open = node.children ? isOpen(node, depth) : false;
    wrapper.append(renderRow(node, depth, open));
    if (open) wrapper.append(renderChildren(node, depth));
    return wrapper;
  }

  function redrawNode(wrapper, node, depth) {
    wrapper.replaceWith(renderNode(node, depth));
  }

  function draw() {
    element.replaceChildren(renderNode(root, 0));
  }

  /** Filtert nach Schlüssel oder Wert; Treffer sind aufgeklappt, ein leerer Text zeigt alles. */
  function setQuery(text) {
    query = text;
    if (query) {
      matches = new Map();
      markMatches(root, false, matches);
    } else {
      matches = null;
    }
    openOverrides.clear();
    draw();
    return matches ? matches.get(root).visible : true;
  }

  draw();
  return { element, setQuery };
}

/**
 * Baum mit Suchfeld in der Kopfleiste. `roots` = `[{ title, node, prefix }]`; mehrere Wurzeln (YAML mit
 * mehreren Dokumenten) stehen als Abschnitte untereinander und teilen sich die Suche.
 */
function createTreeView(roots) {
  const view = document.createElement('div');
  const trees = roots.map((root) => {
    const section = document.createElement('section');
    section.className = 'tree-section';
    if (roots.length > 1) {
      const title = document.createElement('h2');
      title.className = 'tree-title';
      title.textContent = root.title;
      section.append(title);
    }
    const tree = createTree(root.node, root.prefix);
    section.append(tree.element);
    view.append(section);
    return { section, tree };
  });
  const noHits = document.createElement('p');
  noHits.className = 'viewer-note';
  noHits.textContent = 'Keine Treffer.';
  noHits.hidden = true;
  view.append(noHits);
  view.searchInput = addSearchField('Suchen …', (text) => {
    let anyVisible = false;
    for (const { section, tree } of trees) {
      section.hidden = !tree.setQuery(text);
      if (!section.hidden) anyVisible = true;
    }
    noHits.hidden = anyVisible;
  });
  return view;
}
