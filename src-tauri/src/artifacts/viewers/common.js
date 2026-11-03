// Gemeinsame Bausteine der Betrachter für Text: Bibliotheken von cdn.jsdelivr.net mit fester
// Version und Prüfsumme, nummerierte Zeilen, Größen-Deckel, Sprache nach Endung.
const CDN = 'https://cdn.jsdelivr.net/npm/';
const HIGHLIGHT = `${CDN}@highlightjs/cdn-assets@11.9.0/`;
const LIBRARIES = {
  highlight: {
    url: `${HIGHLIGHT}highlight.min.js`,
    integrity: 'sha384-F/bZzf7p3Joyp5psL90p/p89AZJsndkSoGwRpXcZhleCWhd8SnRuoYo4d0yirjJp',
  },
  powershell: {
    url: `${HIGHLIGHT}languages/powershell.min.js`,
    integrity: 'sha384-LWJZQx0dgGhEK7snfNYrQs5K+QKD1sOmE02sOQCz4br9UmqSJDvPLoUVFaUyFnjq',
  },
  dos: {
    url: `${HIGHLIGHT}languages/dos.min.js`,
    integrity: 'sha384-6+/BHm9a7L7KFZE/myAKBddEjwfPovVtBzDrEu/6VqocfSqvRMyM+Lf3k9n89yGH',
  },
  marked: {
    url: `${CDN}marked@12.0.2/marked.min.js`,
    integrity: 'sha384-/TQbtLCAerC3jgaim+N78RZSDYV7ryeoBCVqTuzRrFec2akfBkHS7ACQ3PQhvMVi',
  },
  purify: {
    url: `${CDN}dompurify@3.1.6/dist/purify.min.js`,
    integrity: 'sha384-+VfUPEb0PdtChMwmBcBmykRMDd+v6D/oFmB3rZM/puCMDYcIvF968OimRh4KQY9a',
  },
  mermaid: {
    url: `${CDN}mermaid@10.9.1/dist/mermaid.min.js`,
    integrity: 'sha384-WmdflGW9aGfoBdHc4rRyWzYuAjEmDwMdGdiPNacbwfGKxBW/SO6guzuQ76qjnSlr',
  },
  yaml: {
    url: `${CDN}js-yaml@4.1.0/dist/js-yaml.min.js`,
    integrity: 'sha384-+pxiN6T7yvpryuJmE1gM9PX7yQit15auDb+ZwwvJOd/4be2Cie5/IuVXgQb/S9du',
  },
  papaparse: {
    url: `${CDN}papaparse@5.4.1/papaparse.min.js`,
    integrity: 'sha384-D/t0ZMqQW31H3az8ktEiNb39wyKnS82iFY52QPACM+IjKW3jDUhyIgh2PApRqJZs',
  },
  // ES-Modul, das `globalThis.pdfjsLib` setzt.
  pdfjs: {
    url: `${CDN}pdfjs-dist@4.4.168/build/pdf.min.mjs`,
    integrity: 'sha384-fzqD3KLclV6zyTSuToNuSG370CipiqN572vfT2zti7ORKnQPaJrvsz8deY+/Tgwe',
    isModule: true,
  },
  // Setzt `globalThis.pdfjsWorker`: pdf.js rechnet dann im Hauptthread statt in einem Worker.
  pdfjsWorker: {
    url: `${CDN}pdfjs-dist@4.4.168/build/pdf.worker.min.mjs`,
    integrity: 'sha384-RTiLhGDoA0SEd0K0E9qrg2HwSEEnAo91MwUh79PteR4v01eEByhHOdEkFKxeaeqw',
    isModule: true,
  },
};
const MAX_TEXT_BYTES = 5 * 1024 * 1024;
const MAX_HIGHLIGHT_BYTES = 512 * 1024;
const BIG_FILE_LINES = 3000;
const MAX_TABLE_ROWS = 5000;
const SEARCH_DELAY_MS = 150;

// Endung → highlight.js-Sprache; was fehlt, bleibt ungefärbt.
const LANGUAGES = {
  ts: 'typescript', tsx: 'typescript', js: 'javascript', jsx: 'javascript', mjs: 'javascript',
  py: 'python', rs: 'rust', go: 'go', kt: 'kotlin', java: 'java', cs: 'csharp', sql: 'sql',
  sh: 'bash', ps1: 'powershell', bat: 'dos', cmd: 'dos', css: 'css', scss: 'scss', c: 'c',
  cpp: 'cpp', h: 'c', rb: 'ruby', php: 'php', swift: 'swift', json: 'json', yaml: 'yaml',
  yml: 'yaml', xml: 'xml', html: 'xml', toml: 'ini', ini: 'ini', md: 'markdown',
};

const loadedScripts = new Map();

/** Lädt ein Skript einmal; lehnt ab, wenn es nicht kommt oder die Prüfsumme nicht stimmt. */
function loadScript(url, integrity, isModule = false) {
  if (!loadedScripts.has(url)) {
    loadedScripts.set(
      url,
      new Promise((resolve, reject) => {
        const script = document.createElement('script');
        if (isModule) script.type = 'module';
        script.src = url;
        script.integrity = integrity;
        script.crossOrigin = 'anonymous';
        script.addEventListener('load', () => resolve());
        script.addEventListener('error', () => reject(new Error(url)));
        document.head.append(script);
      }),
    );
  }
  return loadedScripts.get(url);
}

function loadLibrary(name) {
  return loadScript(LIBRARIES[name].url, LIBRARIES[name].integrity, LIBRARIES[name].isModule);
}

function languageOf(file) {
  const dot = file.lastIndexOf('.');
  if (dot < 0) return '';
  return LANGUAGES[file.slice(dot + 1).toLowerCase()] ?? '';
}

/** Höchstens `limit` Zeichen, am Zeilenende abgeschnitten, damit keine halbe Zeile bleibt. */
function capText(text, limit = MAX_TEXT_BYTES) {
  if (text.length <= limit) return { text, truncated: false };
  const cut = text.lastIndexOf('\n', limit);
  return { text: text.slice(0, cut > 0 ? cut : limit), truncated: true };
}

/**
 * Holt die Datei als Text, höchstens `MAX_TEXT_BYTES`: der Rest wird gar nicht erst übertragen.
 * Wirft, wenn der Server die Datei nicht liefert.
 */
async function fetchText() {
  const response = await fetch(viewer.url());
  if (!response.ok) throw new Error(String(response.status));
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let text = '';
  let received = 0;
  let reachedEnd = false;
  while (received < MAX_TEXT_BYTES) {
    const { done, value } = await reader.read();
    if (done) {
      reachedEnd = true;
      break;
    }
    received += value.length;
    text += decoder.decode(value, { stream: true });
  }
  text += decoder.decode();
  if (reachedEnd) return capText(text);
  await reader.cancel();
  const capped = capText(text, text.length - 1);
  return { text: capped.text, truncated: true };
}

function escapeHtml(text) {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

/**
 * Zeilen als `<div class="line" id="L<n>" data-n="<n>">`; die Nummer zeichnet das CSS. `classify`
 * liefert optional eine Klasse je Zeile (Log, Diff). Die Zeilen tragen `data-anchor`, damit ein
 * Klick auf gefärbten Text beim Kommentieren an der Zeile landet.
 */
function lines(text, classify) {
  const rows = text.split(/\r?\n/);
  if (rows.length > 1 && rows[rows.length - 1] === '') rows.pop();
  const digits = String(rows.length).length;
  const stage = document.createElement('div');
  stage.className = rows.length > BIG_FILE_LINES ? 'lines lines--big' : 'lines';
  stage.style.setProperty('--gutter', `calc(${digits}ch + 24px)`);
  const html = [];
  rows.forEach((row, index) => {
    const extra = classify ? classify(row, index) : '';
    const number = index + 1;
    html.push(
      `<div class="line${extra ? ` ${extra}` : ''}" id="L${number}" data-n="${number}" data-anchor>${escapeHtml(row)}</div>`,
    );
  });
  stage.innerHTML = html.join('');
  stage.rows = rows;
  return stage;
}

/** Rohtext mit Zeilennummern, wenn eine Bibliothek fehlt. */
function rawFallback(text, note) {
  const container = lines(text);
  viewer.stage.replaceChildren(container);
  if (note) viewer.setMeta(note);
  return container;
}

/** Hinweis über den Zeilen: die Datei wurde gekürzt. */
function capNotice() {
  const notice = document.createElement('div');
  notice.className = 'cap-notice';
  notice.textContent = 'Datei gekürzt — vollständig über „Im Browser öffnen“';
  return notice;
}

/**
 * Teilt gefärbtes HTML an den Zeilenenden; offene `<span>` werden in der nächsten Zeile
 * wieder geöffnet, damit jede Zeile für sich gültig ist (mehrzeilige Kommentare).
 */
function splitHighlighted(html) {
  const result = [];
  const open = [];
  let current = '';
  const tokens = html.split(/(<span[^>]*>|<\/span>|\n)/);
  for (const token of tokens) {
    if (token === '\n') {
      result.push(current + '</span>'.repeat(open.length));
      current = open.join('');
    } else if (token.startsWith('<span')) {
      open.push(token);
      current += token;
    } else if (token === '</span>') {
      open.pop();
      current += token;
    } else {
      current += token;
    }
  }
  result.push(current + '</span>'.repeat(open.length));
  return result;
}

/** Färbt vorhandene Zeilen an Ort und Stelle, damit Kommentar-Anker bestehen bleiben. */
async function highlightLines(container, language) {
  if (!language || container.rows.join('\n').length > MAX_HIGHLIGHT_BYTES) return false;
  try {
    await loadLibrary('highlight');
    if (LIBRARIES[language]) await loadLibrary(language);
  } catch {
    return false;
  }
  if (!window.hljs.getLanguage(language)) return false;
  const colored = splitHighlighted(
    window.hljs.highlight(container.rows.join('\n'), { language, ignoreIllegals: true }).value,
  );
  const elements = container.children;
  colored.forEach((html, index) => {
    if (elements[index]) elements[index].innerHTML = html;
  });
  return true;
}

/** Färbt die Zeile `number` als Fehlerstelle und rollt sie in die Mitte. */
function markErrorLine(container, number) {
  const element = container.children[number - 1];
  if (!element) return;
  element.style.background = 'var(--diff-del-bg)';
  element.style.color = 'var(--diff-del-fg)';
  element.style.boxShadow = 'inset 3px 0 var(--diff-del-fg)';
  element.scrollIntoView({ block: 'center' });
}

/** Suchfeld in der Kopfleiste; `onChange` bekommt den Suchtext in Kleinbuchstaben, leicht verzögert. */
function addSearchField(placeholder, onChange) {
  const input = document.createElement('input');
  input.type = 'search';
  input.placeholder = placeholder;
  input.setAttribute('aria-label', placeholder);
  Object.assign(input.style, {
    padding: '2px 8px',
    border: '1px solid var(--border)',
    borderRadius: '4px',
    background: 'var(--bg)',
    color: 'var(--fg)',
    font: 'inherit',
  });
  let timer;
  input.addEventListener('input', () => {
    clearTimeout(timer);
    timer = setTimeout(() => onChange(input.value.trim().toLowerCase()), SEARCH_DELAY_MS);
  });
  document.getElementById('viewer-tools').prepend(input);
  return input;
}

/** Sich ausschließende Ansichten als Knöpfe in der Kopfleiste: `views` = `[{ label, show }]`. */
function addViewSwitch(views, initialIndex = 0) {
  const buttons = [];
  const select = (index) => {
    buttons.forEach((button, buttonIndex) => {
      button.setAttribute('aria-pressed', String(buttonIndex === index));
    });
    views[index].show();
  };
  views.forEach((view, index) => {
    buttons.push(viewer.addTool(view.label, () => select(index), index === initialIndex));
  });
  select(initialIndex);
}

/** Tabellen-Stile für CSV und JSON Lines, einmal je Seite. */
function ensureTableStyles() {
  if (document.getElementById('table-styles')) return;
  const style = document.createElement('style');
  style.id = 'table-styles';
  style.textContent = `
    .dt { min-width: 100%; border-collapse: collapse; font: 12px/1.5 var(--font-mono); }
    .dt th, .dt td {
      max-width: 480px; padding: 2px 10px; border-bottom: 1px solid var(--border);
      overflow: hidden; text-align: left; text-overflow: ellipsis; white-space: nowrap; vertical-align: top;
    }
    .dt th {
      position: sticky; top: 0; z-index: 1; background: var(--bg-bar); font-weight: 600;
      cursor: pointer; user-select: none;
    }
    .dt th[aria-sort] { color: var(--accent); }
    .dt .num { text-align: right; }
    .dt .nr { color: var(--fg-muted); text-align: right; user-select: none; }
    .dt .muted { color: var(--fg-muted); }
    .dt tbody tr:hover { background: var(--bg-bar); }
  `;
  document.head.append(style);
}
