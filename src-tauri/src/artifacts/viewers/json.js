// Betrachter für JSON (Baum oder formatierter Rohtext) und JSON Lines (Tabelle, Datensatz als Baum).
const MAX_SCHEMA_RECORDS = 200;
const MAX_CELL_CHARS = 120;
const VALUE_COLUMN = Symbol('value');

(async () => {
  let loaded;
  try {
    loaded = await fetchText();
  } catch {
    viewer.showNote('Die Datei lässt sich nicht laden.');
    return;
  }
  if (viewer.kind === 'json-lines') {
    showJsonLines(loaded);
  } else {
    showJson(loaded);
  }

  function showJson(source) {
    let value;
    try {
      if (source.truncated) throw new Error('Datei zu groß für den Baum, hier gekürzt');
      value = JSON.parse(source.text);
    } catch (error) {
      showInvalidJson(source, error);
      return;
    }
    const root = nodeFromValue(value, '$', '$');
    const tree = createTreeView([{ title: '', node: root, prefix: '' }]);
    const size = viewer.formatSize(new Blob([source.text]).size);
    viewer.setMeta(`${size} · ${countNodes(root)} Werte`);
    let rawView;
    addViewSwitch([
      {
        label: 'Baum',
        show: () => {
          tree.searchInput.hidden = false;
          viewer.stage.replaceChildren(tree);
        },
      },
      {
        label: 'Rohtext',
        show: () => {
          tree.searchInput.hidden = true;
          if (!rawView) {
            rawView = lines(JSON.stringify(value, null, 2));
            viewer.stage.replaceChildren(rawView);
            highlightLines(rawView, 'json');
            return;
          }
          viewer.stage.replaceChildren(rawView);
        },
      },
    ]);
  }

  // Kaputtes JSON: Rohtext, die Fehlerzeile aus der Meldung markiert.
  function showInvalidJson(source, error) {
    const container = lines(source.text);
    viewer.stage.replaceChildren(...(source.truncated ? [capNotice()] : []), container);
    const errorLine = errorLineOf(error.message, source.text);
    if (errorLine) markErrorLine(container, errorLine);
    const where = errorLine ? ` (Zeile ${errorLine})` : '';
    viewer.setMeta(`Kein gültiges JSON${where}: ${error.message}`);
    highlightLines(container, 'json');
  }

  // Neuere Browser nennen in der Meldung nur noch einen Textausschnitt, keine Stelle; dann sucht
  // `firstBrokenLength` sie selbst.
  function errorLineOf(message, text) {
    const explicit = /line (\d+)/.exec(message);
    if (explicit) return Number(explicit[1]);
    const position = /position (\d+)/.exec(message);
    const index = position ? Number(position[1]) : firstBrokenLength(text) - 1;
    if (index < 0) return 0;
    return text.slice(0, index + 1).split('\n').length;
  }

  // Kürzeste Länge, bei der der Anfang des Textes nicht mehr nur „zu früh zu Ende“ ist, sondern
  // schon falsch: ein Anfang, der einmal falsch ist, bleibt es, deshalb reicht die Halbierung.
  function firstBrokenLength(text) {
    const isBroken = (length) => {
      try {
        JSON.parse(text.slice(0, length));
        return false;
      } catch (error) {
        if (/end of JSON|unterminated|unexpected end/i.test(error.message)) return false;
        // „Erwartet ein Schlüssel“ nach einem Komma am Ende meldet die Stelle hinter dem Text.
        const position = /position (\d+)/.exec(error.message);
        if (position) return Number(position[1]) < length;
        return true;
      }
    };
    let low = 1;
    let high = text.length;
    if (!isBroken(high)) return 0;
    while (low < high) {
      const middle = Math.floor((low + high) / 2);
      if (isBroken(middle)) high = middle;
      else low = middle + 1;
    }
    return low;
  }

  function showJsonLines(source) {
    ensureTableStyles();
    const rows = source.text.split(/\r?\n/);
    const records = [];
    rows.forEach((row, index) => {
      if (row.trim() !== '') records.push({ number: index + 1, row });
    });
    const shown = records.slice(0, MAX_TABLE_ROWS).map(parseRecord);
    const columns = columnsOf(shown);
    const table = document.createElement('table');
    table.className = 'dt';
    table.innerHTML = `<thead><tr><th class="nr">#</th>${columns
      .map((column) => `<th>${escapeHtml(columnLabel(column))}</th>`)
      .join('')}</tr></thead>`;
    const body = document.createElement('tbody');
    shown.forEach((record) => body.append(recordRow(record, columns)));
    table.append(body);

    const notices = [];
    if (source.truncated) notices.push(capNotice());
    if (records.length > shown.length) {
      const notice = capNotice();
      notice.textContent = `Zeigt die ersten ${shown.length} von ${records.length} Datensätzen`;
      notices.push(notice);
    }
    viewer.stage.replaceChildren(...notices, table);
    const failed = shown.filter((record) => record.error).length;
    const size = viewer.formatSize(new Blob([source.text]).size);
    const failedNote = failed > 0 ? ` · ${failed} fehlerhaft` : '';
    viewer.setMeta(`${records.length} Datensätze${failedNote} · ${size}`);
  }

  function parseRecord(record) {
    try {
      return { ...record, value: JSON.parse(record.row) };
    } catch (error) {
      return { ...record, error: error.message };
    }
  }

  // Spalten = Vereinigung der Schlüssel der ersten Datensätze; Werte, die keine Objekte sind, bekommen eine Spalte „Wert“.
  function columnsOf(shown) {
    const columns = [];
    const seen = new Set();
    const add = (column) => {
      if (!seen.has(column)) {
        seen.add(column);
        columns.push(column);
      }
    };
    shown
      .filter((record) => !record.error)
      .slice(0, MAX_SCHEMA_RECORDS)
      .forEach((record) => {
        if (isPlainObject(record.value)) {
          Object.keys(record.value).forEach(add);
        } else {
          add(VALUE_COLUMN);
        }
      });
    return columns;
  }

  function isPlainObject(value) {
    return value !== null && typeof value === 'object' && !Array.isArray(value);
  }

  function columnLabel(column) {
    return column === VALUE_COLUMN ? 'Wert' : column;
  }

  function cellHtml(value) {
    if (value === undefined) return '<td></td>';
    if (value === null) return '<td class="muted">null</td>';
    const isObject = typeof value === 'object';
    const text = isObject ? JSON.stringify(value) : String(value);
    const clipped = text.length > MAX_CELL_CHARS ? `${text.slice(0, MAX_CELL_CHARS)}…` : text;
    const className = typeof value === 'number' ? ' class="num"' : '';
    return `<td${className}>${escapeHtml(clipped)}</td>`;
  }

  function recordRow(record, columns) {
    const row = document.createElement('tr');
    row.id = `r${record.number}`;
    row.setAttribute('data-anchor', '');
    if (record.error) {
      row.innerHTML = `<td class="nr">${record.number}</td><td colspan="${Math.max(columns.length, 1)}" style="color: var(--diff-del-fg); background: var(--diff-del-bg)">${escapeHtml(record.error)} — ${escapeHtml(record.row.slice(0, MAX_CELL_CHARS))}</td>`;
      return row;
    }
    const cells = columns.map((column) => {
      if (column === VALUE_COLUMN) {
        return cellHtml(isPlainObject(record.value) ? undefined : record.value);
      }
      return cellHtml(isPlainObject(record.value) ? record.value[column] : undefined);
    });
    row.innerHTML = `<td class="nr">${record.number}</td>${cells.join('')}`;
    row.style.cursor = 'pointer';
    row.addEventListener('click', () => toggleDetail(row, record, columns.length));
    return row;
  }

  // Ein Klick auf den Datensatz klappt darunter seinen Baum auf oder zu.
  function toggleDetail(row, record, columnCount) {
    const next = row.nextElementSibling;
    if (next && next.classList.contains('jl-detail')) {
      next.remove();
      return;
    }
    const detail = document.createElement('tr');
    detail.className = 'jl-detail';
    const cell = document.createElement('td');
    cell.colSpan = columnCount + 1;
    cell.style.cssText = 'max-width: none; white-space: normal;';
    cell.append(createTree(nodeFromValue(record.value, '$', '$'), `r${record.number}`).element);
    detail.append(cell);
    row.after(detail);
  }
})();
