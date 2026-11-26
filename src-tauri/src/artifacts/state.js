// Zustand interaktiver Artefakte (ADR 081). Der Server setzt dieses Skript an den Anfang von <head>,
// vor jedes Skript der Seite. Der Rahmen hat keine eigene Herkunft (ohne allow-same-origin), darum
// wirft localStorage dort; hier ersetzen wir es durch einen Speicher, der schon gefüllt ist und
// jede Änderung an den Verwalter meldet. Der Server ersetzt den Platzhalter in `initial` durch den
// gespeicherten Zustand als JSON.
(() => {
  const MAX_BYTES = 1048576;
  const initial = __STATE__;
  const bytesOf = (map) => new TextEncoder().encode(JSON.stringify(Object.fromEntries(map))).length;

  const storage = (map, onChange) => {
    const text = (value) => String(value);
    return {
      get length() {
        return map.size;
      },
      key: (index) => Array.from(map.keys())[index] ?? null,
      getItem: (key) => map.get(text(key)) ?? null,
      setItem: (key, value) => {
        const next = new Map(map);
        next.set(text(key), text(value));
        if (bytesOf(next) > MAX_BYTES) {
          throw new DOMException('Der Speicher der Seite ist voll (1 MiB).', 'QuotaExceededError');
        }
        map.set(text(key), text(value));
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
