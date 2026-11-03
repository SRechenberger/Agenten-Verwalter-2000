// Ton-Betrachter: Steuerung des Browsers und Wellenform aus `decodeAudioData`; Klick in die
// Wellenform springt an die Stelle.
(async () => {
  const style = document.createElement('style');
  style.textContent = `
    .audio-stage { display: flex; flex-direction: column; gap: 16px; padding: 24px; }
    .audio-stage canvas { width: 100%; height: 160px; cursor: pointer; }
    .audio-stage audio { width: 100%; }
  `;
  document.head.append(style);

  let buffer;
  let blob;
  try {
    const response = await fetch(viewer.url());
    if (!response.ok) throw new Error(String(response.status));
    blob = await response.blob();
  } catch {
    viewer.showNote('Die Tondatei lässt sich nicht laden.');
    return;
  }

  const stage = document.createElement('div');
  stage.className = 'audio-stage';
  const canvas = document.createElement('canvas');
  canvas.setAttribute('aria-label', 'Wellenform');
  const audio = document.createElement('audio');
  audio.controls = true;
  audio.src = URL.createObjectURL(blob);
  stage.append(canvas, audio);
  viewer.stage.replaceChildren(stage);
  audio.addEventListener('error', () => viewer.showNote('Die Tondatei lässt sich hier nicht abspielen.'));

  try {
    const context = new AudioContext();
    buffer = await context.decodeAudioData(await blob.arrayBuffer());
    void context.close();
  } catch {
    canvas.remove();
  }

  const colors = () => {
    const css = getComputedStyle(document.documentElement);
    return { wave: css.getPropertyValue('--fg-muted'), played: css.getPropertyValue('--accent') };
  };

  // Je Pixelspalte Minimum und Maximum — einmal berechnet, beim Abspielen nur neu gemalt.
  let peaks = [];
  const measure = () => {
    if (!buffer) return;
    const width = Math.max(1, Math.floor(canvas.clientWidth * devicePixelRatio));
    canvas.width = width;
    canvas.height = Math.floor(canvas.clientHeight * devicePixelRatio);
    const data = buffer.getChannelData(0);
    const block = Math.max(1, Math.floor(data.length / width));
    peaks = Array.from({ length: width }, (_, column) => {
      let min = 1;
      let max = -1;
      for (let index = column * block; index < (column + 1) * block && index < data.length; index += 1) {
        min = Math.min(min, data[index]);
        max = Math.max(max, data[index]);
      }
      return [min, max];
    });
  };

  const draw = () => {
    if (!buffer) return;
    const context = canvas.getContext('2d');
    const { wave, played } = colors();
    const middle = canvas.height / 2;
    const progress = audio.duration ? audio.currentTime / audio.duration : 0;
    context.clearRect(0, 0, canvas.width, canvas.height);
    peaks.forEach(([min, max], column) => {
      context.fillStyle = column / peaks.length <= progress ? played : wave;
      context.fillRect(column, middle + min * middle, 1, Math.max(1, (max - min) * middle));
    });
  };

  if (buffer) {
    const minutes = Math.floor(buffer.duration / 60);
    const seconds = String(Math.round(buffer.duration % 60)).padStart(2, '0');
    viewer.setMeta(
      `${minutes}:${seconds} · ${buffer.numberOfChannels} Kanal · ${buffer.sampleRate} Hz · ${viewer.formatSize(blob.size)}`,
    );
    measure();
    draw();
    audio.addEventListener('timeupdate', draw);
    window.addEventListener('resize', () => {
      measure();
      draw();
    });
    canvas.addEventListener('click', (event) => {
      if (!audio.duration) return;
      audio.currentTime = (event.offsetX / canvas.clientWidth) * audio.duration;
      draw();
    });
  } else {
    viewer.setMeta(viewer.formatSize(blob.size));
  }
})();
