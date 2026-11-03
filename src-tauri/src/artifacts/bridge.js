// Brücke für Kommentare an Artefakten (ADR 035). Passiv bis `mode`.
(() => {
  if (window.parent === window) return;

  let on = false;
  let hovered = null;
  let markNodes = [];
  let positionQueued = false;

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

  const positionOutline = () => {
    if (!hovered) return;
    const rect = hovered.getBoundingClientRect();
    Object.assign(outline.style, {
      display: 'block', left: `${rect.left}px`, top: `${rect.top}px`, width: `${rect.width}px`, height: `${rect.height}px`,
    });
    tag.textContent = labelOf(hovered);
    Object.assign(tag.style, { display: 'block', left: `${rect.left}px`, top: `${rect.top - 20}px` });
  };

  const clearHover = () => {
    hovered = null;
    outline.style.display = 'none';
    tag.style.display = 'none';
  };

  const positionMarks = () => {
    positionQueued = false;
    markNodes.forEach(({ node, element }) => {
      const rect = element.getBoundingClientRect();
      Object.assign(node.style, { left: `${rect.right - 10}px`, top: `${rect.top - 10}px` });
    });
    positionOutline();
  };

  const queuePosition = () => {
    if (!positionQueued) {
      positionQueued = true;
      requestAnimationFrame(positionMarks);
    }
  };

  const setMarks = (marks) => {
    markNodes.forEach(({ node }) => node.remove());
    markNodes = [];
    if (!Array.isArray(marks)) return;
    marks.forEach((mark) => {
      if (!mark || typeof mark.n !== 'number' || typeof mark.selector !== 'string') return;
      let element;
      try { element = document.querySelector(mark.selector); } catch { return; }
      if (!element) return;
      const node = overlayStyle(document.createElement('div'));
      node.textContent = String(mark.n);
      Object.assign(node.style, {
        width: '20px', height: '20px', borderRadius: '50%', background: '#c4572f', color: '#fff',
        display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: '11px', fontWeight: '700',
      });
      markNodes.push({ node, element });
    });
    queuePosition();
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
  window.addEventListener('scroll', queuePosition, true);
  window.addEventListener('resize', queuePosition);
  window.addEventListener('load', () => {
    window.parent.postMessage({ verwalter: 1, type: 'ready' }, '*');
  });
})();
