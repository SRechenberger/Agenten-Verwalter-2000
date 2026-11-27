// Zustand interaktiver Artefakte (ADR 081). Der Server setzt dieses Skript an den Anfang von <head>,
// vor jedes Skript der Seite. Der Rahmen hat keine eigene Herkunft (ohne allow-same-origin), darum
// wirft localStorage dort; hier ersetzen wir es durch einen Speicher, der schon gefüllt ist und
// jede Änderung an den Verwalter meldet. Der Server ersetzt den Platzhalter in `initial` durch den
// JSON-Text des gespeicherten Zustands als Zeichenkette: `JSON.parse` behält auch `__proto__` als Schlüssel.
(() => {
  const MAX_BYTES = 1048576;
  const MAX_MARKS = 32;
  const BUFFER_PREFIX = 'verwalter-zustand:';
  const initial = JSON.parse(__STATE__);
  const bytesOf = (map) => new TextEncoder().encode(JSON.stringify(Object.fromEntries(map))).length;
  // Prüfsumme eines Stands (FNV-1a über die sortierten Einträge): gleich bei gleichen Schlüsseln und Werten.
  const markOf = (object) => {
    const text = JSON.stringify(Object.entries(object).sort(([a], [b]) => (a < b ? -1 : 1)));
    let hash = 0x811c9dc5;
    for (let index = 0; index < text.length; index += 1) {
      hash = Math.imul(hash ^ text.charCodeAt(index), 0x01000193);
    }
    return (hash >>> 0).toString(36);
  };

  // Wie Storage: Methoden und `length` gehen beim Lesen vor, jede andere Eigenschaft ist ein Schlüssel
  // (`localStorage.x = 'y'`, `delete localStorage.x`, `Object.keys(localStorage)`). Ein Aufruf ohne
  // Änderung meldet nichts, wie bei Storage.
  const storage = (map, onChange) => {
    const text = (value) => String(value);
    const methods = {
      get length() {
        return map.size;
      },
      key: (index) => Array.from(map.keys())[index] ?? null,
      getItem: (key) => map.get(text(key)) ?? null,
      setItem: (key, value) => {
        const name = text(key);
        const next = text(value);
        if (map.get(name) === next) return;
        if (bytesOf(new Map(map).set(name, next)) > MAX_BYTES) {
          throw new DOMException('Der Speicher der Seite ist voll (1 MiB).', 'QuotaExceededError');
        }
        map.set(name, next);
        onChange();
      },
      removeItem: (key) => {
        if (map.delete(text(key))) onChange();
      },
      clear: () => {
        if (map.size === 0) return;
        map.clear();
        onChange();
      },
    };
    return new Proxy(methods, {
      get: (target, key) => (key in target ? Reflect.get(target, key) : map.get(key)),
      set: (target, key, value) => {
        if (typeof key === 'symbol') return Reflect.set(target, key, value);
        target.setItem(key, value);
        return true;
      },
      has: (target, key) => key in target || map.has(key),
      deleteProperty: (target, key) => {
        if (typeof key !== 'symbol') target.removeItem(key);
        return true;
      },
      ownKeys: () => Array.from(map.keys()),
      getOwnPropertyDescriptor: (_target, key) =>
        map.has(key)
          ? { value: map.get(key), writable: true, enumerable: true, configurable: true }
          : undefined,
    });
  };

  // Stand beim Seitenwechsel: Lädt die nächste Seite im Rahmen (Website, Neu laden), während die Meldung noch
  // gespeichert wird, setzt der Server den Stand von davor ein. Darum merkt sich der Rahmen in seinem Namen
  // (`window.name` übersteht die Navigation im Rahmen, ein neuer Rahmen beginnt leer) den letzten eigenen Stand
  // und die Prüfsummen der Stände, auf denen er aufbaut: den geladenen und jeden gemeldeten. Zeigt die Datei einen
  // davon außer dem letzten, ist sie nicht neuer: Es gilt der eigene Stand, und er wird erneut gemeldet. Sonst
  // gilt die Datei. Bestätigt der Rahmen das Speichern des neuesten eigenen Stands, leert die Seite den Namen:
  // Jedes spätere Dokument im Rahmen könnte ihn lesen.
  const buffer = (() => {
    if (!window.name.startsWith(BUFFER_PREFIX)) return null;
    try {
      const value = JSON.parse(window.name.slice(BUFFER_PREFIX.length));
      return Array.isArray(value.marks) && value.data !== null && typeof value.data === 'object' ? value : null;
    } catch {
      return null;
    }
  })();
  const fileMark = markOf(initial);
  const isFileBehind =
    buffer !== null && buffer.marks.includes(fileMark) && buffer.marks[buffer.marks.length - 1] !== fileMark;
  if (buffer !== null && !isFileBehind) window.name = '';
  let marks = isFileBehind ? buffer.marks : [fileMark];
  const saved = new Map(Object.entries(isFileBehind ? buffer.data : initial));
  // Ohne Verzögerung: ein Wechsel in den Chat direkt nach dem Klick darf nichts verlieren.
  const report = () => {
    const data = Object.fromEntries(saved);
    const mark = markOf(data);
    if (window.parent !== window) {
      window.parent.postMessage({ verwalter: 1, type: 'state', data, mark }, '*');
    }
    return { data, mark };
  };
  const persistent = storage(saved, () => {
    const { data, mark } = report();
    marks = [...marks, mark].slice(-MAX_MARKS);
    window.name = BUFFER_PREFIX + JSON.stringify({ data, marks });
  });
  if (isFileBehind) report();
  // Eine Bestätigung für einen älteren Stand leert nichts; auch die nächste Seite im Rahmen bekommt sie.
  window.addEventListener('message', (event) => {
    const message = event.data;
    if (event.source !== window.parent || message?.verwalter !== 1 || message.type !== 'state-saved') return;
    if (message.mark === marks[marks.length - 1] && window.name.startsWith(BUFFER_PREFIX)) window.name = '';
  });
  const volatile = storage(new Map(), () => {});

  // Greift das Überschreiben nicht, läuft die Seite weiter wie ohne Zustand.
  try {
    Object.defineProperty(window, 'localStorage', { value: persistent, configurable: true });
    Object.defineProperty(window, 'sessionStorage', { value: volatile, configurable: true });
  } catch (error) {
    console.error('Zustand des Artefakts nicht bereitgestellt', error);
  }
})();
