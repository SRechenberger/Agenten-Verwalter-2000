// Ordner-Betrachter: Dateibaum links, rechts die Vorschau der gewählten Datei in einem eigenen
// abgeschotteten Rahmen (mit dem passenden Betrachter). Besteht der Ordner nur aus Bildern, zeigt er
// ein Raster mit Vorschaubildern. Die Liste kommt von `?list=1` desselben Ordners.
(async () => {
  const INDENT_PX = 14;
  // Dieselben Rechte wie der äußere Rahmen der App (ArtifactFrame): eigene, leere Herkunft.
  const FRAME_SANDBOX = 'allow-scripts allow-forms allow-modals';
  const PREVIEW_KINDS = new Set([
    'html', 'markdown', 'json', 'json-lines', 'yaml', 'config', 'xml', 'csv', 'code', 'text', 'log', 'diff',
    'image', 'video', 'audio', 'pdf', 'mermaid',
  ]);
  const PREFERRED_START = /^(index\.html?|readme\.md)$/i;

  const style = document.createElement('style');
  style.textContent = `
    .fo-layout { position: absolute; inset: 0; display: flex; }
    .fo-tree { flex: none; width: 260px; overflow: auto; padding: 6px 0; border-right: 1px solid var(--border); background: var(--bg-bar); }
    @media (max-width: 720px) { .fo-tree { width: 40%; } }
    .fo-row { display: flex; gap: 6px; align-items: baseline; width: 100%; padding: 2px 10px; border: 0; background: none; color: var(--fg); font: 12px/1.5 var(--font-mono); text-align: left; cursor: pointer; }
    .fo-row:hover { background: var(--bg); }
    .fo-row[aria-current='true'] { background: var(--bg); color: var(--accent); }
    .fo-row__name { overflow-wrap: anywhere; }
    .fo-row__size { margin-left: auto; flex: none; color: var(--fg-muted); font-size: 11px; }
    .fo-row--folder { font-weight: 600; }
    .fo-preview { flex: 1; min-width: 0; border: 0; background: var(--bg); }
    .fo-empty { flex: 1; padding: 24px; color: var(--fg-muted); }
    .fo-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 12px; padding: 16px; }
    .fo-tile { display: block; padding: 6px; border: 1px solid var(--border); border-radius: 4px; background: var(--bg-bar); color: var(--fg); font: 11px var(--font-sans); text-align: center; cursor: pointer; }
    .fo-tile:hover { border-color: var(--accent); }
    .fo-tile img { display: block; width: 100%; height: 120px; margin-bottom: 4px; object-fit: contain; background: var(--bg); }
    .fo-tile span { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  `;
  document.head.append(style);

  let listing;
  try {
    const response = await fetch('?list=1');
    if (!response.ok) throw new Error(String(response.status));
    listing = await response.json();
  } catch {
    viewer.showNote('Der Ordner lässt sich nicht lesen.');
    return;
  }
  const entries = listing.entries;
  const files = entries.filter((entry) => entry.kind !== 'folder');
  if (files.length === 0) {
    viewer.showNote('Der Ordner ist leer.');
    return;
  }
  const fileCount = `${files.length} ${files.length === 1 ? 'Datei' : 'Dateien'}`;
  const totalSize = viewer.formatSize(files.reduce((sum, entry) => sum + entry.size, 0));
  const cutNote = listing.truncated ? ' · Liste gekürzt' : '';
  viewer.setMeta(`${fileCount} · ${totalSize}${cutNote}`);

  const isGallery = files.every((entry) => entry.kind === 'image');
  if (isGallery) {
    showGallery();
  } else {
    showBrowser();
  }

  function urlOf(path) {
    return path.split('/').map(encodeURIComponent).join('/');
  }

  // HTML läuft direkt, alles mit Betrachter als dessen Seite; der Rest hat keine Vorschau.
  function previewUrl(entry) {
    if (entry.kind === 'html') return urlOf(entry.path);
    if (PREVIEW_KINDS.has(entry.kind)) return `${urlOf(entry.path)}?view=1`;
    return null;
  }

  function createPreview(entry) {
    const url = previewUrl(entry);
    if (!url) {
      const note = document.createElement('p');
      note.className = 'fo-empty';
      note.textContent = `Keine Vorschau für ${entry.path} (${viewer.formatSize(entry.size)}) — oben „Mit Standardprogramm öffnen“ nutzen.`;
      return note;
    }
    const frame = document.createElement('iframe');
    frame.className = 'fo-preview';
    frame.src = url;
    frame.setAttribute('sandbox', FRAME_SANDBOX);
    frame.setAttribute('allow', 'fullscreen');
    frame.title = entry.path;
    return frame;
  }

  function showGallery(backButton) {
    viewer.stage.replaceChildren();
    viewer.stage.style.position = '';
    const grid = document.createElement('div');
    grid.className = 'fo-grid';
    files.forEach((entry, index) => {
      const tile = document.createElement('button');
      tile.type = 'button';
      tile.className = 'fo-tile';
      tile.id = `f${index}`;
      tile.setAttribute('data-anchor', '');
      const image = document.createElement('img');
      image.loading = 'lazy';
      image.alt = entry.path;
      image.src = urlOf(entry.path);
      const name = document.createElement('span');
      name.textContent = entry.path;
      tile.append(image, name);
      tile.addEventListener('click', () => showImage(entry));
      grid.append(tile);
    });
    viewer.stage.append(grid);
    backButton?.remove();
  }

  // Ein Bild aus dem Raster in groß, im Bild-Betrachter; „Raster“ führt zurück.
  function showImage(entry) {
    viewer.stage.style.position = 'relative';
    viewer.stage.replaceChildren(createPreview(entry));
    viewer.stage.firstChild.style.cssText = 'position:absolute;inset:0;width:100%;height:100%';
    const back = viewer.addTool('‹ Raster', () => showGallery(back));
  }

  function showBrowser() {
    viewer.stage.style.position = 'relative';
    viewer.stage.style.overflow = 'hidden';
    const tree = document.createElement('nav');
    tree.className = 'fo-tree';
    const preview = document.createElement('div');
    preview.style.cssText = 'flex:1;min-width:0;display:flex';
    const layout = document.createElement('div');
    layout.className = 'fo-layout';
    layout.append(tree, preview);
    viewer.stage.replaceChildren(layout);

    const collapsed = new Set();
    let filter = '';
    let selected = null;

    addSearchField('Dateien filtern', (text) => {
      filter = text;
      drawTree();
    });
    select(files.find((entry) => PREFERRED_START.test(entry.path)) ?? files[0]);
    drawTree();

    function select(entry) {
      selected = entry;
      preview.replaceChildren(createPreview(entry));
      tree.querySelectorAll('.fo-row').forEach((row) => {
        row.setAttribute('aria-current', String(row.dataset.path === entry.path));
      });
    }

    function isHidden(entry) {
      const parts = entry.path.split('/');
      return parts.slice(0, -1).some((_, index) => collapsed.has(parts.slice(0, index + 1).join('/')));
    }

    function visibleEntries() {
      if (filter) return files.filter((entry) => entry.path.toLowerCase().includes(filter));
      return entries.filter((entry) => !isHidden(entry));
    }

    function drawTree() {
      const rows = visibleEntries().map((entry) => createRow(entry));
      tree.replaceChildren(...rows);
      if (rows.length === 0) tree.textContent = 'Keine Treffer.';
    }

    function createRow(entry) {
      const isFolder = entry.kind === 'folder';
      const parts = entry.path.split('/');
      const row = document.createElement('button');
      row.type = 'button';
      row.className = isFolder ? 'fo-row fo-row--folder' : 'fo-row';
      row.dataset.path = entry.path;
      row.id = `f${entries.indexOf(entry)}`;
      row.setAttribute('data-anchor', '');
      row.style.paddingLeft = `${10 + (filter ? 0 : (parts.length - 1) * INDENT_PX)}px`;
      const name = document.createElement('span');
      name.className = 'fo-row__name';
      const marker = collapsed.has(entry.path) ? '▸ ' : '▾ ';
      name.textContent = isFolder ? `${marker}${parts[parts.length - 1]}` : filter ? entry.path : parts[parts.length - 1];
      row.append(name);
      if (!isFolder) {
        const size = document.createElement('span');
        size.className = 'fo-row__size';
        size.textContent = viewer.formatSize(entry.size);
        row.append(size);
      }
      row.setAttribute('aria-current', String(entry === selected));
      row.addEventListener('click', () => {
        if (!isFolder) {
          select(entry);
          return;
        }
        if (collapsed.has(entry.path)) collapsed.delete(entry.path);
        else collapsed.add(entry.path);
        drawTree();
      });
      return row;
    }
  }
})();
