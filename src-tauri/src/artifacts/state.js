// Zustand interaktiver Artefakte (ADR 081). Der Server setzt dieses Skript an den Anfang von <head>,
// vor jedes Skript der Seite. Der Rahmen hat keine eigene Herkunft (ohne allow-same-origin), darum
// wirft localStorage dort; hier ersetzen wir es durch einen Speicher, der schon gefüllt ist und
// jede Änderung an den Verwalter meldet. Der Server ersetzt den Platzhalter in `initial` durch den
// JSON-Text des gespeicherten Zustands als Zeichenkette: `JSON.parse` behält auch `__proto__` als Schlüssel.
(() => {
  const MAX_BYTES = 1048576;
  const initial = JSON.parse(__STATE__);
  const bytesOf = (map) => new TextEncoder().encode(JSON.stringify(Object.fromEntries(map))).length;

  // Wie Storage: Methoden und `length` gehen beim Lesen vor, jede andere Eigenschaft ist ein Schlüssel
  // (`localStorage.x = 'y'`, `delete localStorage.x`, `Object.keys(localStorage)`). Ein Aufruf ohne
  // Änderung meldet nichts, sonst laden zwei offene Ansichten einander endlos neu.
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

  const saved = new Map(Object.entries(initial));
  const persistent = storage(saved, () => {
    // Ohne Verzögerung: ein Wechsel in den Chat direkt nach dem Klick darf nichts verlieren.
    if (window.parent !== window) {
      window.parent.postMessage({ verwalter: 1, type: 'state', data: Object.fromEntries(saved) }, '*');
    }
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
