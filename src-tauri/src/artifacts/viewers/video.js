// Video-Betrachter: Steuerung des Browsers, Geschwindigkeit, Bild vor/zurück mit `.`/`,` in der
// Pause.
(() => {
  const FRAME_SECONDS = 1 / 30;
  const style = document.createElement('style');
  style.textContent = `
    .video-stage { display: flex; height: 100%; padding: 16px; background: #000; }
    .video-stage video { margin: auto; max-width: 100%; max-height: 100%; }
  `;
  document.head.append(style);

  const stage = document.createElement('div');
  stage.className = 'video-stage';
  const video = document.createElement('video');
  video.controls = true;
  video.preload = 'metadata';
  video.src = viewer.url();
  stage.append(video);
  viewer.stage.replaceChildren(stage);

  video.addEventListener('loadedmetadata', () => {
    const minutes = Math.floor(video.duration / 60);
    const seconds = String(Math.round(video.duration % 60)).padStart(2, '0');
    viewer.setMeta(`${video.videoWidth} × ${video.videoHeight} px · ${minutes}:${seconds}`);
  });
  video.addEventListener('error', () => viewer.showNote('Das Video lässt sich hier nicht abspielen.'));

  const speedButtons = [0.5, 1, 1.5, 2].map((rate) =>
    viewer.addTool(
      `${String(rate).replace('.', ',')}×`,
      (clicked) => {
        video.playbackRate = rate;
        for (const button of speedButtons) {
          button.setAttribute('aria-pressed', String(button === clicked));
        }
      },
      rate === 1,
    ),
  );

  document.addEventListener('keydown', (event) => {
    if (!video.paused || (event.key !== '.' && event.key !== ',')) return;
    event.preventDefault();
    const step = event.key === '.' ? FRAME_SECONDS : -FRAME_SECONDS;
    video.currentTime = Math.min(video.duration || 0, Math.max(0, video.currentTime + step));
  });
})();
