// Markdown-Betrachter: marked → DOMPurify → DOM. Überschriften mit Inhaltsverzeichnis, Code mit
// Syntaxfarben, Mermaid-Blöcke als Diagramm, Umschalter „Formatiert | Rohtext“.
(async () => {
  const style = document.createElement('style');
  style.textContent = `
    .md-layout { display: flex; align-items: flex-start; min-height: 100%; }
    .md-toc { position: sticky; top: 0; flex: none; width: 220px; max-height: 100vh; overflow: auto; padding: 16px 8px 16px 16px; border-right: 1px solid var(--border); font-size: 12px; }
    .md-toc a { display: block; padding: 2px 0; color: var(--fg-muted); text-decoration: none; overflow-wrap: anywhere; }
    .md-toc a:hover { color: var(--accent); }
    .md-toc a[data-level='2'] { padding-left: 10px; }
    .md-toc a[data-level='3'], .md-toc a[data-level='4'] { padding-left: 20px; }
    @media (max-width: 720px) { .md-toc { display: none; } }
    .md { flex: 1; min-width: 0; max-width: 860px; margin: 0 auto; padding: 16px 24px 48px; font-size: 14px; line-height: 1.6; overflow-wrap: anywhere; }
    .md h1, .md h2, .md h3, .md h4 { margin: 1.4em 0 0.5em; line-height: 1.25; }
    .md h1 { font-size: 1.8em; padding-bottom: 0.2em; border-bottom: 1px solid var(--border); }
    .md h2 { font-size: 1.4em; padding-bottom: 0.2em; border-bottom: 1px solid var(--border); }
    .md h3 { font-size: 1.15em; }
    .md a { color: var(--accent); }
    .md code { padding: 1px 4px; border-radius: 3px; background: var(--bg-bar); font: 12px var(--font-mono); }
    .md pre { overflow: auto; padding: 10px 12px; border: 1px solid var(--border); border-radius: 4px; background: var(--bg-bar); }
    .md pre code { padding: 0; background: none; }
    .md blockquote { margin: 0.8em 0; padding: 0 12px; border-left: 3px solid var(--border); color: var(--fg-muted); }
    .md table { border-collapse: collapse; display: block; overflow: auto; }
    .md th, .md td { padding: 4px 10px; border: 1px solid var(--border); }
    .md th { background: var(--bg-bar); }
    .md img { max-width: 100%; }
    .md hr { border: 0; border-top: 1px solid var(--border); }
    .md-mermaid { margin: 1em 0; text-align: center; overflow: auto; }
    .md input[type='checkbox'] { margin-right: 6px; }
    .md li.task-list-item { list-style: none; margin-left: -1.4em; }
  `;
  document.head.append(style);

  let loaded;
  try {
    loaded = await fetchText();
  } catch {
    viewer.showNote('Die Datei lässt sich nicht laden.');
    return;
  }

  const rawButton = viewer.addTool('Rohtext', () => showView('raw'), false);
  const formattedButton = viewer.addTool('Formatiert', () => showView('formatted'), true);
  const rawView = lines(loaded.text);
  let formattedView = null;
  const headingCount = (loaded.text.match(/^#{1,6}\s/gm) ?? []).length;
  viewer.setMeta(`${viewer.formatSize(new Blob([loaded.text]).size)} · ${headingCount} Überschriften`);

  try {
    await Promise.all([loadLibrary('marked'), loadLibrary('purify')]);
  } catch {
    rawButton.disabled = true;
    formattedButton.disabled = true;
    formattedButton.setAttribute('aria-pressed', 'false');
    rawFallback(loaded.text, 'Bibliotheken nicht erreichbar — Rohtext');
    return;
  }
  formattedView = await buildFormatted();
  showView('formatted');

  function showView(name) {
    const isRaw = name === 'raw';
    rawButton.setAttribute('aria-pressed', String(isRaw));
    formattedButton.setAttribute('aria-pressed', String(!isRaw));
    const notice = loaded.truncated ? [capNotice()] : [];
    viewer.stage.replaceChildren(...notice, isRaw ? rawView : formattedView);
  }

  async function buildFormatted() {
    const html = window.DOMPurify.sanitize(window.marked.parse(loaded.text, { gfm: true }));
    const body = document.createElement('article');
    body.className = 'md';
    body.innerHTML = html;
    numberBlocks(body);
    const headings = anchorHeadings(body);
    openLinksOutside(body);
    markTasks(body);
    await renderCode(body);

    const layout = document.createElement('div');
    layout.className = 'md-layout';
    if (headings.length >= 3) layout.append(tableOfContents(headings));
    layout.append(body);
    return layout;
  }

  // Absätze, Listenpunkte und Tabellenzeilen als Kommentar-Anker.
  function numberBlocks(root) {
    root.querySelectorAll('p, li, tr, pre, blockquote').forEach((block, index) => {
      block.id = `b${index + 1}`;
      block.setAttribute('data-anchor', '');
    });
  }

  function anchorHeadings(root) {
    const used = new Set();
    const headings = [];
    root.querySelectorAll('h1, h2, h3, h4').forEach((heading) => {
      const base =
        heading.textContent
          .trim()
          .toLowerCase()
          .replace(/[^\p{L}\p{N}]+/gu, '-')
          .replace(/^-+|-+$/g, '') || 'abschnitt';
      let id = base;
      for (let suffix = 2; used.has(id); suffix += 1) id = `${base}-${suffix}`;
      used.add(id);
      heading.id = id;
      heading.setAttribute('data-anchor', '');
      headings.push(heading);
    });
    return headings;
  }

  function tableOfContents(headings) {
    const toc = document.createElement('nav');
    toc.className = 'md-toc';
    headings.forEach((heading) => {
      const link = document.createElement('a');
      link.href = `#${heading.id}`;
      link.dataset.level = heading.tagName.slice(1);
      link.textContent = heading.textContent;
      link.addEventListener('click', (event) => {
        event.preventDefault();
        heading.scrollIntoView();
      });
      toc.append(link);
    });
    return toc;
  }

  // Popups sind gesperrt (ADR 026): der Link bleibt sichtbar, ein Klick im Rahmen tut nichts Schädliches.
  function openLinksOutside(root) {
    root.querySelectorAll('a[href]').forEach((link) => {
      const href = link.getAttribute('href');
      if (href.startsWith('#')) {
        link.addEventListener('click', (event) => {
          event.preventDefault();
          root.querySelector(`[id="${CSS.escape(href.slice(1))}"]`)?.scrollIntoView();
        });
        return;
      }
      link.target = '_blank';
      link.rel = 'noopener noreferrer';
    });
  }

  function markTasks(root) {
    root.querySelectorAll('li > input[type="checkbox"], li > p:first-child > input[type="checkbox"]').forEach(
      (box) => {
        box.disabled = true;
        box.closest('li').classList.add('task-list-item');
      },
    );
  }

  async function renderCode(root) {
    const blocks = Array.from(root.querySelectorAll('pre > code'));
    const mermaidBlocks = blocks.filter((code) => code.classList.contains('language-mermaid'));
    const codeBlocks = blocks.filter((code) => !mermaidBlocks.includes(code));
    await Promise.all([colorCode(codeBlocks), drawDiagrams(mermaidBlocks)]);
  }

  async function colorCode(codeBlocks) {
    if (codeBlocks.length === 0) return;
    try {
      await loadLibrary('highlight');
    } catch {
      return;
    }
    for (const code of codeBlocks) {
      const name = (/language-([\w-]+)/.exec(code.className) ?? [])[1];
      const language = name ? languageAlias(name) : '';
      if (LIBRARIES[language]) {
        try {
          await loadLibrary(language);
        } catch {
          continue;
        }
      }
      if (!language || !window.hljs.getLanguage(language)) continue;
      code.innerHTML = window.hljs.highlight(code.textContent, { language, ignoreIllegals: true }).value;
    }
  }

  // Fence-Namen wie „ts“ oder „powershell“ auf die Namen von highlight.js abbilden.
  function languageAlias(name) {
    const lower = name.toLowerCase();
    return languageOf(`x.${lower}`) || lower;
  }

  async function drawDiagrams(mermaidBlocks) {
    if (mermaidBlocks.length === 0) return;
    try {
      await loadLibrary('mermaid');
    } catch {
      return;
    }
    const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    window.mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', theme: dark ? 'dark' : 'default' });
    let counter = 0;
    for (const code of mermaidBlocks) {
      counter += 1;
      const id = `mermaid-${counter}`;
      try {
        const { svg } = await window.mermaid.render(id, code.textContent);
        const holder = document.createElement('div');
        holder.className = 'md-mermaid';
        holder.innerHTML = svg;
        const pre = code.parentElement;
        holder.id = pre.id;
        holder.setAttribute('data-anchor', '');
        pre.replaceWith(holder);
      } catch {
        // Ungültiger Block: bleibt als Code stehen; mermaid lässt sein Fehler-Element im Body zurück.
        document.getElementById(`d${id}`)?.remove();
      }
    }
  }
})();
