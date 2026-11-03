// Mermaid-Betrachter: Diagramm mit Zoom (Strg + Mausrad, Knöpfe) und Verschieben (Ziehen),
// Umschalter „Diagramm | Quelltext“. Knoten und Gruppen tragen `data-anchor` als Kommentar-Anker.
(async () => {
  const ZOOM_STEP = 1.25;
  const MIN_SCALE = 0.1;
  const MAX_SCALE = 8;
  const FIT_MARGIN = 0.94;

  let loaded;
  try {
    loaded = await fetchText();
  } catch {
    viewer.showNote('Die Datei lässt sich nicht laden.');
    return;
  }
  viewer.setMeta(`${viewer.formatSize(new Blob([loaded.text]).size)} · ${loaded.text.split(/\r?\n/).length} Zeilen`);
  const sourceView = lines(loaded.text);
  const notice = loaded.truncated ? [capNotice()] : [];

  try {
    await loadLibrary('mermaid');
  } catch {
    rawFallback(loaded.text, 'Mermaid nicht erreichbar — Quelltext');
    return;
  }

  const diagram = await drawDiagram(loaded.text);
  if (!diagram.ok) {
    viewer.setMeta(`Kein gültiges Diagramm: ${diagram.message}`);
    viewer.stage.replaceChildren(...notice, sourceView);
    return;
  }
  const zoomTools = addZoomTools(diagram);
  addViewSwitch([
    {
      label: 'Diagramm',
      show: () => {
        zoomTools.forEach((button) => (button.hidden = false));
        viewer.stage.replaceChildren(diagram.frame);
        diagram.fit();
      },
    },
    {
      label: 'Quelltext',
      show: () => {
        zoomTools.forEach((button) => (button.hidden = true));
        viewer.stage.replaceChildren(...notice, sourceView);
      },
    },
  ]);

  async function drawDiagram(source) {
    const isDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    window.mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', theme: isDark ? 'dark' : 'default' });
    let svg;
    try {
      ({ svg } = await window.mermaid.render('diagram', source));
    } catch (error) {
      // Mermaid lässt bei einem Fehler sein Fehler-Element im Body zurück.
      document.getElementById('ddiagram')?.remove();
      return { ok: false, message: String(error?.message ?? error).split('\n')[0] };
    }
    const frame = document.createElement('div');
    frame.style.cssText = 'position:absolute;inset:0;overflow:hidden;cursor:grab;touch-action:none';
    const canvas = document.createElement('div');
    canvas.style.cssText = 'position:absolute;left:0;top:0;transform-origin:0 0';
    canvas.innerHTML = svg;
    frame.append(canvas);
    const element = canvas.querySelector('svg');
    const box = element.viewBox.baseVal;
    const size = box.width > 0 ? { width: box.width, height: box.height } : element.getBoundingClientRect();
    element.setAttribute('width', String(size.width));
    element.setAttribute('height', String(size.height));
    element.style.maxWidth = 'none';
    markAnchors(element);

    const view = { scale: 1, x: 0, y: 0 };
    const apply = () => {
      canvas.style.transform = `translate(${view.x}px, ${view.y}px) scale(${view.scale})`;
    };
    // Zoom um einen Punkt der Bühne: der Punkt unter dem Zeiger bleibt, wo er ist.
    const zoomAt = (factor, pointX, pointY) => {
      const next = Math.min(MAX_SCALE, Math.max(MIN_SCALE, view.scale * factor));
      const ratio = next / view.scale;
      view.x = pointX - (pointX - view.x) * ratio;
      view.y = pointY - (pointY - view.y) * ratio;
      view.scale = next;
      apply();
    };
    const zoomCentered = (factor) => zoomAt(factor, frame.clientWidth / 2, frame.clientHeight / 2);
    const fit = () => {
      const scale = Math.min(frame.clientWidth / size.width, frame.clientHeight / size.height, 1) * FIT_MARGIN;
      view.scale = scale;
      view.x = (frame.clientWidth - size.width * scale) / 2;
      view.y = (frame.clientHeight - size.height * scale) / 2;
      apply();
    };

    frame.addEventListener(
      'wheel',
      (event) => {
        if (!event.ctrlKey) return;
        event.preventDefault();
        const rect = frame.getBoundingClientRect();
        zoomAt(event.deltaY < 0 ? ZOOM_STEP : 1 / ZOOM_STEP, event.clientX - rect.left, event.clientY - rect.top);
      },
      { passive: false },
    );
    frame.addEventListener('pointerdown', (event) => {
      if (event.button !== 0) return;
      let last = { x: event.clientX, y: event.clientY };
      frame.setPointerCapture(event.pointerId);
      frame.style.cursor = 'grabbing';
      const move = (moveEvent) => {
        view.x += moveEvent.clientX - last.x;
        view.y += moveEvent.clientY - last.y;
        last = { x: moveEvent.clientX, y: moveEvent.clientY };
        apply();
      };
      const end = () => {
        frame.style.cursor = 'grab';
        frame.removeEventListener('pointermove', move);
        frame.removeEventListener('pointerup', end);
        frame.removeEventListener('pointercancel', end);
      };
      frame.addEventListener('pointermove', move);
      frame.addEventListener('pointerup', end);
      frame.addEventListener('pointercancel', end);
    });
    window.addEventListener('resize', fit);
    return { ok: true, frame, fit, zoomIn: () => zoomCentered(ZOOM_STEP), zoomOut: () => zoomCentered(1 / ZOOM_STEP) };
  }

  // Die `id`s stammen von Mermaid (`flowchart-A-0`); sie bleiben Anker, auch nach „Neu laden“.
  function markAnchors(element) {
    element.querySelectorAll('.node[id], .cluster[id], .edgeLabel, .actor, .note, .task').forEach((part) => {
      part.setAttribute('data-anchor', '');
    });
  }

  function addZoomTools(rendered) {
    return [
      viewer.addTool('−', rendered.zoomOut),
      viewer.addTool('+', rendered.zoomIn),
      viewer.addTool('Einpassen', rendered.fit),
    ];
  }
})();
