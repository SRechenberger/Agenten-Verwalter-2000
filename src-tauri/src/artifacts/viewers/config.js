// Betrachter für TOML, INI und .env: Quelltext mit Syntaxfarben, Abschnitte `[name]` als Sprungmarken.
// Bei .env stehen die Werte verdeckt (••••) und decken sich je Zeile per Klick auf.
const HIDDEN_VALUE = '••••';
const SECTION_LINE = /^\s*\[{1,2}([^\]]+)\]{1,2}\s*$/;
const ENV_ASSIGNMENT = /^(\s*(?:export\s+)?[A-Za-z_][A-Za-z0-9_.-]*\s*=)(.*)$/;

(async () => {
  let loaded;
  try {
    loaded = await fetchText();
  } catch {
    viewer.showNote('Die Datei lässt sich nicht laden.');
    return;
  }

  const lowerName = viewer.file.toLowerCase();
  const isEnv = lowerName.startsWith('.env') || lowerName.endsWith('.env');
  const container = lines(loaded.text);
  const notices = loaded.truncated ? [capNotice()] : [];
  const navigation = isEnv ? null : sectionNavigation(container.rows);
  viewer.stage.replaceChildren(...notices, ...(navigation ? [navigation] : []), container);

  const size = viewer.formatSize(new Blob([loaded.text]).size);
  const count = container.rows.length;
  viewer.setMeta(`${count} ${count === 1 ? 'Zeile' : 'Zeilen'} · ${size}${isEnv ? ' · Werte verdeckt' : ''}`);

  if (isEnv) {
    maskValues(container);
  } else {
    await highlightLines(container, 'ini');
  }

  /** Eine Leiste mit einem Knopf je Abschnitt; `null`, wenn die Datei keine Abschnitte hat. */
  function sectionNavigation(rows) {
    const sections = [];
    rows.forEach((row, index) => {
      const match = SECTION_LINE.exec(row);
      if (match) sections.push({ name: match[1].trim(), number: index + 1 });
    });
    if (sections.length === 0) return null;
    const bar = document.createElement('nav');
    bar.className = 'cap-notice';
    bar.setAttribute('aria-label', 'Abschnitte');
    bar.style.display = 'flex';
    bar.style.flexWrap = 'wrap';
    bar.style.gap = '4px';
    for (const section of sections) {
      const button = document.createElement('button');
      button.type = 'button';
      button.textContent = section.name;
      Object.assign(button.style, {
        padding: '0 8px',
        border: '1px solid var(--border)',
        borderRadius: '4px',
        background: 'var(--bg)',
        color: 'var(--fg)',
        font: 'inherit',
        cursor: 'pointer',
      });
      button.addEventListener('click', () => {
        document.getElementById(`L${section.number}`).scrollIntoView({ block: 'start' });
      });
      bar.append(button);
    }
    return bar;
  }

  // Die echten Werte liegen nur in diesem Feld, nicht im DOM: ein Kommentar auf die Zeile trägt sie so nicht weiter.
  function maskValues(target) {
    const secrets = new Map();
    target.rows.forEach((row, index) => {
      const element = target.children[index];
      const assignment = ENV_ASSIGNMENT.exec(row);
      if (!assignment) {
        if (row.trim().startsWith('#')) element.style.color = 'var(--code-comment)';
        return;
      }
      const value = assignment[2];
      const key = document.createElement('span');
      key.className = 'hljs-attr';
      key.textContent = assignment[1];
      element.replaceChildren(key);
      if (value.trim() === '') return;
      secrets.set(index, value);
      const hidden = document.createElement('span');
      hidden.textContent = HIDDEN_VALUE;
      hidden.style.cursor = 'pointer';
      hidden.style.color = 'var(--code-string)';
      hidden.title = 'Klicken zum Aufdecken';
      hidden.addEventListener('click', () => {
        const isHidden = hidden.textContent === HIDDEN_VALUE;
        hidden.textContent = isHidden ? secrets.get(index) : HIDDEN_VALUE;
      });
      element.append(hidden);
    });
  }
})();
