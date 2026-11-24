// Brücke für Kommentare an Artefakten (ADR 035). Passiv bis `mode`.
(() => {
  // HTML-Artefakte folgen dem Schema der App wie die Betrachter (ADR 047): `scheme=light|dark` in der
  // Adresse wird zu `data-theme` an `<html>`. Ohne den Parameter bleibt ein Attribut der Seite stehen.
  const scheme = new URLSearchParams(location.search).get('scheme');
  if (scheme === 'light' || scheme === 'dark') document.documentElement.dataset.theme = scheme;

  if (window.parent === window) return;

  let on = false;
  let hovered = null;
  let markNodes = [];
  let positionQueued = false;
  const MARK_SIZE = 20;

  const overlayStyle = (node) => {
    Object.assign(node.style, {
      position: 'fixed',
      pointerEvents: 'none',
      zIndex: '2147483647',
      boxSizing: 'border-box',
      fontFamily: 'system-ui, sans-serif',
    });
    document.documentElement.append(node);
    return node;
  };

  const outline = overlayStyle(document.createElement('div'));
  Object.assign(outline.style, {
    display: 'none',
    border: '2px solid #c4572f',
    margin: '-2px',
  });
  const tag = overlayStyle(document.createElement('div'));
  Object.assign(tag.style, {
    display: 'none',
    padding: '2px 5px',
    borderRadius: '3px',
    background: '#c4572f',
    color: '#fff',
    fontSize: '11px',
    lineHeight: '14px',
    whiteSpace: 'nowrap',
  });

  // Ein Betrachter markiert Blöcke mit `data-anchor`: der Klick auf gefärbten Text darin gilt dem Block.
  const targetOf = (element) => element.closest('[data-anchor]') || element;
  const compact = (value, limit) => value.replace(/\s+/g, ' ').trim().slice(0, limit);
  const tagName = (element) => element.tagName.toLowerCase();

  const selectorPart = (element) => {
    const firstClass = Array.from(element.classList).find((name) => /^[A-Za-z0-9_-]+$/.test(name));
    let position = 1;
    let sibling = element.previousElementSibling;
    while (sibling) {
      if (tagName(sibling) === tagName(element)) position += 1;
      sibling = sibling.previousElementSibling;
    }
    return `${tagName(element)}${firstClass ? `.${firstClass}` : ''}:nth-of-type(${position})`;
  };

  const selectorOf = (element) => {
    if (element.id) return `#${element.id}`;
    const parts = [];
    let current = element;
    while (current && current !== document.body && parts.length < 8) {
      parts.unshift(selectorPart(current));
      current = current.parentElement;
    }
    return parts.join(' > ');
  };

  const labelOf = (element) => `${tagName(element)}${element.classList.length ? ` · ${element.classList[0]}` : ''}`;

  // Ohne Scrollleiste: `innerWidth` zählte sie mit, und eine Marke läge darunter.
  const viewport = () => ({ width: document.documentElement.clientWidth, height: document.documentElement.clientHeight });

  const positionOutline = () => {
    if (!hovered) return;
    const rect = hovered.getBoundingClientRect();
    Object.assign(outline.style, {
      display: 'block', left: `${rect.left}px`, top: `${rect.top}px`, width: `${rect.width}px`, height: `${rect.height}px`,
    });
    tag.textContent = labelOf(hovered);
    tag.style.display = 'block';
    // Das Schild bleibt an der Ecke des Elements, liegt aber ganz im Fenster (ADR 035).
    const { width, height } = viewport();
    Object.assign(tag.style, {
      left: `${Math.max(0, Math.min(rect.left, width - tag.offsetWidth))}px`,
      top: `${Math.min(Math.max(rect.top - 20, 0), Math.max(0, height - tag.offsetHeight))}px`,
    });
  };

  const clearHover = () => {
    hovered = null;
    outline.style.display = 'none';
    tag.style.display = 'none';
  };

  const positionMarks = () => {
    positionQueued = false;
    // Die Marke sitzt an der Ecke ihres Elements. Ragt das Element ins Fenster, wird sie ins Fenster
    // geschoben: Ein Element bis an den Rand ließe sie sonst halb abgeschnitten (ADR 035). Die Marke
    // eines Elements außerhalb des Fensters bleibt unbegrenzt und scrollt mit ihm aus dem Bild.
    const { width, height } = viewport();
    markNodes.forEach(({ node, element }) => {
      const rect = element.getBoundingClientRect();
      const left = rect.right - 10;
      const top = rect.top - 10;
      const visible = rect.bottom > 0 && rect.top < height && rect.right > 0 && rect.left < width;
      Object.assign(node.style, visible
        ? {
          left: `${Math.max(0, Math.min(left, width - MARK_SIZE - 2))}px`,
          top: `${Math.min(Math.max(top, 0), Math.max(0, height - MARK_SIZE))}px`,
        }
        : { left: `${left}px`, top: `${top}px` });
    });
    positionOutline();
  };

  const queuePosition = () => {
    if (!positionQueued) {
      positionQueued = true;
      requestAnimationFrame(positionMarks);
    }
  };

  // Die zuletzt empfangenen Marken. Ein Betrachter baut sein DOM oft erst nach dem Laden und beim
  // Umschalten von Roh auf formatiert neu: Ändert sich `body`, werden sie neu gesetzt. Die Overlays
  // hängen an `documentElement`, lösen den Beobachter also nicht aus.
  let lastMarks = [];
  let observer = null;
  let remarkQueued = false;

  const applyMarks = () => {
    markNodes.forEach(({ node }) => node.remove());
    markNodes = [];
    lastMarks.forEach((mark) => {
      let element;
      try { element = document.querySelector(mark.selector); } catch { return; }
      if (!element) return;
      const node = overlayStyle(document.createElement('div'));
      node.textContent = String(mark.n);
      Object.assign(node.style, {
        width: `${MARK_SIZE}px`, height: `${MARK_SIZE}px`, borderRadius: '50%', background: '#c4572f', color: '#fff',
        display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: '11px', fontWeight: '700',
      });
      markNodes.push({ node, element });
    });
    // Gleich positionieren: Nach einer Änderung unter `body` stünden die Marken sonst einen Frame
    // lang an ihrer statischen Stelle.
    positionMarks();
  };

  const setMarks = (marks) => {
    lastMarks = Array.isArray(marks)
      ? marks.filter((mark) => mark && typeof mark.n === 'number' && typeof mark.selector === 'string')
      : [];
    applyMarks();
    if (observer || !document.body) return;
    observer = new MutationObserver(() => {
      if (!lastMarks.length || remarkQueued) return;
      remarkQueued = true;
      requestAnimationFrame(() => {
        remarkQueued = false;
        applyMarks();
      });
    });
    observer.observe(document.body, { childList: true, subtree: true });
  };

  window.addEventListener('mouseover', (event) => {
    if (!on || !(event.target instanceof Element)) return;
    hovered = targetOf(event.target);
    positionOutline();
  }, true);
  window.addEventListener('mouseout', (event) => {
    if (!on || event.relatedTarget === hovered) return;
    clearHover();
  }, true);
  window.addEventListener('click', (event) => {
    if (!on || !(event.target instanceof Element)) return;
    event.preventDefault();
    event.stopPropagation();
    const element = targetOf(event.target);
    const rect = element.getBoundingClientRect();
    window.parent.postMessage({
      verwalter: 1, type: 'pick', selector: selectorOf(element),
      snippet: compact(element.outerHTML, 300), label: compact(element.textContent || '', 80),
      rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
    }, '*');
  }, true);
  window.addEventListener('message', (event) => {
    const data = event.data;
    if (event.source !== window.parent || !data || data.verwalter !== 1) return;
    if (data.type === 'mode' && typeof data.on === 'boolean') {
      on = data.on;
      if (!on) clearHover();
    }
    if (data.type === 'marks') setMarks(data.marks);
  });
  // Links nach außen öffnet der Verwalter im Standardbrowser: der Rahmen darf weder Popups noch das
  // App-Fenster öffnen, und ohne diese Weiterleitung lief die Navigation je nach WebView ins Leere
  // oder im Rahmen selbst. Verweise auf andere Artefakte (gleicher Server) bleiben im Rahmen.
  const serverOrigin = new URL(location.href).origin;
  const OUTSIDE_PROTOCOLS = ['http:', 'https:', 'mailto:'];
  const openOutside = (event) => {
    if (on || event.defaultPrevented || event.button > 1 || !(event.target instanceof Element)) return;
    const link = event.target.closest('a[href]');
    if (!link) return;
    let url;
    try { url = new URL(link.getAttribute('href'), location.href); } catch { return; }
    if (url.origin === serverOrigin || !OUTSIDE_PROTOCOLS.includes(url.protocol)) return;
    event.preventDefault();
    window.parent.postMessage({ verwalter: 1, type: 'open', url: url.href }, '*');
  };
  window.addEventListener('click', openOutside);
  window.addEventListener('auxclick', openOutside);
  // Der Ordner-Betrachter zeigt Dateien in eigenen Rahmen: deren Links reicht er nach oben durch,
  // aber nur aus Rahmen dieses Servers — eingebettete Fremdseiten öffnen nichts.
  window.addEventListener('message', (event) => {
    const data = event.data;
    if (event.source === window.parent || !data || data.verwalter !== 1 || data.type !== 'open') return;
    const isOwnFrame = Array.from(document.querySelectorAll('iframe')).some(
      (frame) => frame.contentWindow === event.source && new URL(frame.src, location.href).origin === serverOrigin,
    );
    if (isOwnFrame && typeof data.url === 'string') {
      window.parent.postMessage({ verwalter: 1, type: 'open', url: data.url }, '*');
    }
  });
  window.addEventListener('scroll', queuePosition, true);
  window.addEventListener('resize', queuePosition);
  window.addEventListener('load', () => {
    window.parent.postMessage({ verwalter: 1, type: 'ready' }, '*');
  });
})();
