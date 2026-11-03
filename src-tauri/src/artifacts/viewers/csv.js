// Betrachter für CSV und TSV (papaparse): Trennzeichen erkannt, feste Kopfzeile, Sortieren per Klick auf
// den Spaltenkopf, Filter über alle Spalten. Ohne Bibliothek bleibt der Rohtext.
const NUMBER_PATTERN = /^-?\d+(?:[.,]\d+)?$/;
const NUMERIC_SAMPLE_ROWS = 500;
const DELIMITER_NAMES = { ',': 'Komma', ';': 'Semikolon', '\t': 'Tab' };

(async () => {
  let loaded;
  try {
    loaded = await fetchText();
  } catch {
    viewer.showNote('Die Datei lässt sich nicht laden.');
    return;
  }
  try {
    await loadLibrary('papaparse');
  } catch {
    rawFallback(loaded.text, 'Bibliothek nicht erreichbar — Rohtext');
    return;
  }
  ensureTableStyles();

  const isTsv = /\.tsv$/i.test(viewer.file);
  const parsed = window.Papa.parse(loaded.text, {
    delimiter: isTsv ? '\t' : '',
    delimitersToGuess: [',', ';', '\t'],
    skipEmptyLines: 'greedy',
  });
  if (parsed.data.length === 0) {
    viewer.showNote('Die Datei ist leer.');
    return;
  }

  const header = parsed.data[0];
  const entries = parsed.data.slice(1).map((cells, index) => ({ cells, number: index + 1 }));
  const columnCount = parsed.data.reduce((widest, cells) => Math.max(widest, cells.length), 0);
  const isNumeric = numericColumns();
  let sortColumn = -1;
  let sortDescending = false;
  let filterText = '';

  const table = document.createElement('table');
  table.className = 'dt';
  const head = document.createElement('thead');
  const body = document.createElement('tbody');
  table.append(head, body);
  const notice = capNotice();
  notice.hidden = true;
  viewer.stage.replaceChildren(...(loaded.truncated ? [capNotice()] : []), notice, table);

  const delimiterName = DELIMITER_NAMES[parsed.meta.delimiter] ?? `„${parsed.meta.delimiter}“`;
  const size = viewer.formatSize(new Blob([loaded.text]).size);
  viewer.setMeta(`${columnCount} Spalten · ${entries.length} Zeilen · Trenner ${delimiterName} · ${size}`);
  addSearchField('Filtern …', (text) => {
    filterText = text;
    drawBody();
  });
  head.addEventListener('click', (event) => {
    const cell = event.target.closest('th[data-column]');
    if (cell) sortBy(Number(cell.dataset.column));
  });
  drawHead();
  drawBody();

  function numericColumns() {
    const flags = [];
    for (let column = 0; column < columnCount; column += 1) {
      const values = entries
        .slice(0, NUMERIC_SAMPLE_ROWS)
        .map((entry) => (entry.cells[column] ?? '').trim())
        .filter((value) => value !== '');
      flags.push(values.length > 0 && values.every((value) => NUMBER_PATTERN.test(value)));
    }
    return flags;
  }

  function toNumber(value) {
    return Number(value.trim().replace(',', '.'));
  }

  // Erst aufsteigend, dann absteigend, dann wieder die Reihenfolge der Datei.
  function sortBy(column) {
    if (sortColumn !== column) {
      sortColumn = column;
      sortDescending = false;
    } else if (!sortDescending) {
      sortDescending = true;
    } else {
      sortColumn = -1;
    }
    drawHead();
    drawBody();
  }

  function compareEntries(left, right) {
    const leftValue = (left.cells[sortColumn] ?? '').trim();
    const rightValue = (right.cells[sortColumn] ?? '').trim();
    if (leftValue === '' || rightValue === '') {
      // Leere Zellen stehen immer hinten, egal in welcher Richtung.
      return (leftValue === '' ? 1 : 0) - (rightValue === '' ? 1 : 0);
    }
    const order = isNumeric[sortColumn]
      ? toNumber(leftValue) - toNumber(rightValue)
      : leftValue.localeCompare(rightValue, 'de', { numeric: true });
    return sortDescending ? -order : order;
  }

  function visibleEntries() {
    let result = entries;
    if (filterText) {
      result = result.filter((entry) =>
        entry.cells.some((cell) => cell.toLowerCase().includes(filterText)),
      );
    }
    if (sortColumn >= 0) result = [...result].sort(compareEntries);
    return result;
  }

  function drawHead() {
    const cells = ['<th class="nr">#</th>'];
    for (let column = 0; column < columnCount; column += 1) {
      const name = (header[column] ?? '').trim() || `Spalte ${column + 1}`;
      const sorted = column === sortColumn;
      let arrow = '';
      let sortAttribute = '';
      if (sorted) {
        arrow = sortDescending ? ' ▼' : ' ▲';
        sortAttribute = ` aria-sort="${sortDescending ? 'descending' : 'ascending'}"`;
      }
      const className = isNumeric[column] ? ' class="num"' : '';
      cells.push(`<th data-column="${column}"${className}${sortAttribute}>${escapeHtml(name)}${arrow}</th>`);
    }
    head.innerHTML = `<tr>${cells.join('')}</tr>`;
  }

  function drawBody() {
    const matching = visibleEntries();
    const shown = matching.slice(0, MAX_TABLE_ROWS);
    body.innerHTML = shown.map(rowHtml).join('');
    notice.hidden = matching.length <= shown.length;
    notice.textContent = `Zeigt die ersten ${shown.length} von ${matching.length} Zeilen`;
  }

  function rowHtml(entry) {
    const cells = [`<td class="nr">${entry.number}</td>`];
    for (let column = 0; column < columnCount; column += 1) {
      const className = isNumeric[column] ? ' class="num"' : '';
      cells.push(`<td${className}>${escapeHtml(entry.cells[column] ?? '')}</td>`);
    }
    return `<tr id="r${entry.number}" data-anchor>${cells.join('')}</tr>`;
  }
})();
