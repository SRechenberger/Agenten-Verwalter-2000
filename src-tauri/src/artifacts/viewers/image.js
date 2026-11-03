// Bild-Betrachter: Einpassen/1:1 per Klick, Zoom mit Strg+Mausrad, Schachbrett hinter
// Transparenz, Hintergrund hell/dunkel.
(async () => {
  const style = document.createElement('style');
  style.textContent = `
    .image-stage { display: flex; min-height: 100%; padding: 16px; }
    .image-stage--dark { background: #111; }
    .image-stage--light { background: #fff; }
    .image-frame {
      margin: auto;
      background-color: #fff;
      background-image: linear-gradient(45deg, #ddd 25%, transparent 25%),
        linear-gradient(-45deg, #ddd 25%, transparent 25%),
        linear-gradient(45deg, transparent 75%, #ddd 75%),
        linear-gradient(-45deg, transparent 75%, #ddd 75%);
      background-size: 16px 16px;
      background-position: 0 0, 0 8px, 8px -8px, -8px 0;
      line-height: 0;
    }
    .image-frame img { display: block; cursor: zoom-in; }
    .image-frame--fit img { max-width: calc(100vw - 32px); max-height: calc(100vh - 80px); }
    .image-frame--zoomed img { cursor: zoom-out; }
  `;
  document.head.append(style);

  let blob;
  try {
    const response = await fetch(viewer.url());
    if (!response.ok) throw new Error(String(response.status));
    blob = await response.blob();
  } catch {
    viewer.showNote('Das Bild lässt sich nicht laden.');
    return;
  }

  const stage = document.createElement('div');
  stage.className = 'image-stage image-stage--light';
  const frame = document.createElement('div');
  frame.className = 'image-frame image-frame--fit';
  const image = document.createElement('img');
  image.alt = viewer.file;
  frame.append(image);
  stage.append(frame);
  viewer.stage.replaceChildren(stage);

  let scale = 0;
  const apply = () => {
    frame.classList.toggle('image-frame--fit', scale === 0);
    frame.classList.toggle('image-frame--zoomed', scale !== 0);
    image.style.width = scale === 0 ? '' : `${image.naturalWidth * scale}px`;
    fitButton.setAttribute('aria-pressed', String(scale === 0));
  };

  image.addEventListener('load', () => {
    const dimensions = image.naturalWidth ? `${image.naturalWidth} × ${image.naturalHeight} px · ` : '';
    viewer.setMeta(`${dimensions}${viewer.formatSize(blob.size)}`);
  });
  image.addEventListener('error', () => viewer.showNote('Das Bild lässt sich nicht anzeigen.'));
  image.addEventListener('click', () => {
    scale = scale === 0 ? 1 : 0;
    apply();
  });
  stage.addEventListener(
    'wheel',
    (event) => {
      if (!event.ctrlKey) return;
      event.preventDefault();
      const current = scale === 0 ? image.width / (image.naturalWidth || 1) : scale;
      scale = Math.min(16, Math.max(0.05, current * (event.deltaY < 0 ? 1.25 : 0.8)));
      apply();
    },
    { passive: false },
  );

  const fitButton = viewer.addTool(
    'Einpassen',
    () => {
      scale = 0;
      apply();
    },
    true,
  );
  viewer.addTool('1:1', () => {
    scale = 1;
    apply();
  });
  viewer.addTool(
    'Dunkel',
    (button) => {
      const dark = stage.classList.toggle('image-stage--dark');
      stage.classList.toggle('image-stage--light', !dark);
      button.setAttribute('aria-pressed', String(dark));
    },
    false,
  );

  image.src = URL.createObjectURL(blob);
})();
