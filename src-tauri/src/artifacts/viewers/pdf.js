// PDF-Betrachter (pdf.js): Seiten untereinander auf <canvas> mit Textebene, Vorschaubilder links,
// Zoom und Suche. Gezeichnet wird nur, was nah am sichtbaren Bereich liegt. Jede Seite ist als
// `#p<n>` ein Kommentar-Anker.
(async () => {
  const MIN_SCALE = 0.25;
  const MAX_SCALE = 4;
  const ZOOM_STEP = 1.2;
  const PAGE_PADDING = 32;
  const THUMB_WIDTH = 112;
  const RENDER_MARGIN = '900px 0px';

  const style = document.createElement('style');
  style.textContent = `
    .pdf-layout { position: absolute; inset: 0; display: flex; }
    .pdf-thumbs { flex: none; width: ${THUMB_WIDTH + 28}px; overflow: auto; padding: 10px 0; border-right: 1px solid var(--border); background: var(--bg-bar); }
    @media (max-width: 720px) { .pdf-thumbs { display: none; } }
    .pdf-thumb { display: block; margin: 0 auto 10px; padding: 3px; border: 2px solid transparent; border-radius: 3px; background: none; color: var(--fg-muted); font: 11px var(--font-sans); text-align: center; cursor: pointer; }
    .pdf-thumb[aria-current='true'] { border-color: var(--accent); color: var(--accent); }
    .pdf-thumb canvas { display: block; margin-bottom: 2px; background: #fff; }
    .pdf-pages { flex: 1; min-width: 0; overflow: auto; padding: ${PAGE_PADDING / 2}px; background: var(--bg-bar); }
    .pdf-page { position: relative; margin: 0 auto 12px; background: #fff; box-shadow: 0 1px 4px rgba(0, 0, 0, 0.35); }
    .pdf-page canvas { display: block; }
    .pdf-page__note { padding: 12px; color: #6e6e6e; }
    .textLayer { position: absolute; inset: 0; overflow: clip; line-height: 1; text-align: initial; text-size-adjust: none; forced-color-adjust: none; transform-origin: 0 0; }
    .textLayer :is(span, br) { position: absolute; color: transparent; white-space: pre; cursor: text; transform-origin: 0% 0%; }
    .textLayer ::selection { background: rgba(0, 90, 255, 0.3); }
    .textLayer span.hit { background: rgba(255, 190, 0, 0.5); }
    .textLayer .endOfContent { display: block; position: absolute; inset: 100% 0 0; z-index: -1; cursor: default; user-select: none; }
  `;
  document.head.append(style);

  try {
    await Promise.all([loadLibrary('pdfjs'), loadLibrary('pdfjsWorker')]);
  } catch {
    viewer.showNote('PDF-Anzeige nicht geladen — oben „Im Browser öffnen“ nutzen.');
    return;
  }
  const pdfjs = globalThis.pdfjsLib;

  let pdf;
  let byteCount = 0;
  try {
    const response = await fetch(viewer.url());
    if (!response.ok) throw new Error(String(response.status));
    const bytes = new Uint8Array(await response.arrayBuffer());
    byteCount = bytes.length;
    // Kein Worker: in der Sandbox mit der CSP des Servers startet ein Worker nicht und meldet auch
    // keinen Fehler, `getDocument` bliebe hängen. Mit geladenem `pdfjsWorker` rechnet pdf.js im Hauptthread.
    pdf = await pdfjs.getDocument({ data: bytes }).promise;
  } catch (error) {
    viewer.showNote(
      error?.name === 'PasswordException' ? 'Das PDF ist passwortgeschützt.' : 'Das PDF lässt sich nicht lesen.',
    );
    return;
  }

  const firstSize = (await pdf.getPage(1)).getViewport({ scale: 1 });
  const entries = [];
  let scale = 1;
  let query = '';
  let currentNumber = 1;

  const thumbs = document.createElement('nav');
  thumbs.className = 'pdf-thumbs';
  const scroller = document.createElement('div');
  scroller.className = 'pdf-pages';
  const layout = document.createElement('div');
  layout.className = 'pdf-layout';
  layout.append(thumbs, scroller);
  viewer.stage.style.position = 'relative';
  viewer.stage.style.overflow = 'hidden';
  viewer.stage.replaceChildren(layout);

  for (let number = 1; number <= pdf.numPages; number += 1) {
    const wrapper = document.createElement('div');
    wrapper.className = 'pdf-page';
    wrapper.id = `p${number}`;
    wrapper.setAttribute('data-anchor', '');
    const thumb = document.createElement('button');
    thumb.type = 'button';
    thumb.className = 'pdf-thumb';
    thumb.textContent = String(number);
    thumb.addEventListener('click', () => goTo(number));
    entries.push({ number, wrapper, thumb, size: firstSize, status: 'idle', generation: 0, thumbDone: false, task: null });
    scroller.append(wrapper);
    thumbs.append(thumb);
  }

  const zoomOutButton = viewer.addTool('−', () => zoomBy(1 / ZOOM_STEP));
  const zoomInButton = viewer.addTool('+', () => zoomBy(ZOOM_STEP));
  viewer.addTool('Breite', fitWidth);
  const searchInput = addSearchField('Im PDF suchen', (text) => search(text));
  searchInput.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') jumpToNextHit();
  });
  zoomOutButton.title = 'Verkleinern';
  zoomInButton.title = 'Vergrößern';

  const pageObserver = new IntersectionObserver(
    (changes) => {
      changes.forEach((change) => {
        const entry = entries[Number(change.target.id.slice(1)) - 1];
        if (change.isIntersecting) renderPage(entry);
        else releasePage(entry);
      });
    },
    { root: scroller, rootMargin: RENDER_MARGIN },
  );
  const thumbObserver = new IntersectionObserver(
    (changes) => {
      changes.forEach((change) => {
        if (change.isIntersecting) renderThumb(entries[Number(change.target.dataset.number) - 1]);
      });
    },
    { root: thumbs, rootMargin: '300px 0px' },
  );

  layoutPages();
  fitWidth();
  entries.forEach((entry) => {
    entry.thumb.dataset.number = String(entry.number);
    thumbObserver.observe(entry.thumb);
    pageObserver.observe(entry.wrapper);
  });
  scroller.addEventListener('scroll', trackCurrentPage, { passive: true });
  window.addEventListener('resize', () => trackCurrentPage());
  showMeta();

  function showMeta() {
    viewer.setMeta(`Seite ${currentNumber} / ${pdf.numPages} · ${viewer.formatSize(byteCount)}${hitSummary()}`);
  }

  function sizeOf(entry) {
    return { width: entry.size.width * scale, height: entry.size.height * scale };
  }

  function layoutPages() {
    entries.forEach((entry) => {
      const { width, height } = sizeOf(entry);
      entry.wrapper.style.width = `${width}px`;
      entry.wrapper.style.height = `${height}px`;
    });
  }

  function fitWidth() {
    setScale((scroller.clientWidth - PAGE_PADDING) / firstSize.width);
  }

  function zoomBy(factor) {
    setScale(scale * factor);
  }

  // Der Ausschnitt bleibt, wo er war: gemerkt wird der Anteil der Gesamthöhe, nicht der Pixelwert.
  function setScale(next) {
    const clamped = Math.min(MAX_SCALE, Math.max(MIN_SCALE, next));
    if (!(clamped > 0) || clamped === scale) return;
    const ratio = scroller.scrollHeight > 0 ? scroller.scrollTop / scroller.scrollHeight : 0;
    scale = clamped;
    entries.forEach(releasePage);
    layoutPages();
    scroller.scrollTop = ratio * scroller.scrollHeight;
    entries.forEach((entry) => {
      if (isNearViewport(entry)) renderPage(entry);
    });
  }

  function isNearViewport(entry) {
    const top = entry.wrapper.offsetTop - scroller.scrollTop;
    return top < scroller.clientHeight + 900 && top + entry.wrapper.offsetHeight > -900;
  }

  function goTo(number) {
    scroller.scrollTo({ top: entries[number - 1].wrapper.offsetTop - PAGE_PADDING / 2 });
  }

  function trackCurrentPage() {
    const middle = scroller.scrollTop + scroller.clientHeight / 3;
    const current = entries.findLast((entry) => entry.wrapper.offsetTop <= middle) ?? entries[0];
    if (current.number === currentNumber) return;
    currentNumber = current.number;
    entries.forEach((entry) => {
      entry.thumb.setAttribute('aria-current', String(entry === current));
    });
    current.thumb.scrollIntoView({ block: 'nearest' });
    showMeta();
  }

  async function renderPage(entry) {
    if (entry.status !== 'idle') return;
    entry.status = 'rendering';
    entry.generation += 1;
    const generation = entry.generation;
    const isStale = () => entry.generation !== generation;
    try {
      const page = await pdf.getPage(entry.number);
      if (isStale()) return;
      entry.size = page.getViewport({ scale: 1 });
      const viewport = page.getViewport({ scale });
      entry.wrapper.style.width = `${viewport.width}px`;
      entry.wrapper.style.height = `${viewport.height}px`;
      const pixelRatio = window.devicePixelRatio || 1;
      const canvas = document.createElement('canvas');
      canvas.width = Math.floor(viewport.width * pixelRatio);
      canvas.height = Math.floor(viewport.height * pixelRatio);
      canvas.style.width = `${viewport.width}px`;
      canvas.style.height = `${viewport.height}px`;
      entry.task = page.render({
        canvasContext: canvas.getContext('2d'),
        viewport,
        transform: pixelRatio === 1 ? null : [pixelRatio, 0, 0, pixelRatio, 0, 0],
      });
      await entry.task.promise;
      if (isStale()) return;
      const textDiv = document.createElement('div');
      textDiv.className = 'textLayer';
      textDiv.style.setProperty('--scale-factor', String(viewport.scale));
      entry.wrapper.replaceChildren(canvas, textDiv);
      await new pdfjs.TextLayer({ textContentSource: page.streamTextContent(), container: textDiv, viewport }).render();
      if (isStale()) return;
      entry.textDiv = textDiv;
      entry.status = 'done';
      markHits(entry);
    } catch (error) {
      if (error?.name === 'RenderingCancelledException' || isStale()) return;
      entry.status = 'failed';
      const note = document.createElement('div');
      note.className = 'pdf-page__note';
      note.textContent = 'Diese Seite lässt sich nicht darstellen.';
      entry.wrapper.replaceChildren(note);
    }
  }

  // Seiten weit außerhalb des Fensters geben ihr Bild frei: ein langes PDF füllt sonst den Speicher.
  function releasePage(entry) {
    if (entry.status === 'idle') return;
    entry.generation += 1;
    entry.task?.cancel();
    entry.task = null;
    entry.textDiv = null;
    entry.status = 'idle';
    entry.wrapper.replaceChildren();
  }

  async function renderThumb(entry) {
    if (entry.thumbDone) return;
    entry.thumbDone = true;
    try {
      const page = await pdf.getPage(entry.number);
      const viewport = page.getViewport({ scale: THUMB_WIDTH / page.getViewport({ scale: 1 }).width });
      const canvas = document.createElement('canvas');
      canvas.width = Math.floor(viewport.width);
      canvas.height = Math.floor(viewport.height);
      await page.render({ canvasContext: canvas.getContext('2d'), viewport }).promise;
      entry.thumb.prepend(canvas);
    } catch {
      entry.thumbDone = false;
    }
  }

  const pageTexts = new Map();
  let hitCounts = new Map();

  async function textOf(number) {
    if (!pageTexts.has(number)) {
      const page = await pdf.getPage(number);
      const content = await page.getTextContent();
      pageTexts.set(number, content.items.map((item) => item.str).join(' ').toLowerCase());
    }
    return pageTexts.get(number);
  }

  async function search(text) {
    query = text;
    hitCounts = new Map();
    entries.forEach(markHits);
    if (!query) {
      showMeta();
      return;
    }
    for (const entry of entries) {
      if (query !== text) return;
      const pageText = await textOf(entry.number);
      const count = pageText.split(query).length - 1;
      if (count > 0) hitCounts.set(entry.number, count);
    }
    if (query !== text) return;
    showMeta();
    const first = [...hitCounts.keys()].find((number) => number >= currentNumber) ?? [...hitCounts.keys()][0];
    if (first) goTo(first);
  }

  function jumpToNextHit() {
    const pages = [...hitCounts.keys()];
    if (pages.length === 0) return;
    goTo(pages.find((number) => number > currentNumber) ?? pages[0]);
  }

  // Die gefundenen Stellen färbt die Textebene (Spans, die den Suchtext enthalten).
  function markHits(entry) {
    if (!entry.textDiv) return;
    entry.textDiv.querySelectorAll('span').forEach((span) => {
      span.classList.toggle('hit', query !== '' && span.textContent.toLowerCase().includes(query));
    });
  }

  function hitSummary() {
    if (!query) return '';
    const total = [...hitCounts.values()].reduce((sum, count) => sum + count, 0);
    return total === 0 ? ' · kein Treffer' : ` · ${total} Treffer auf ${hitCounts.size} Seiten`;
  }
})();
