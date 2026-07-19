import '@cheetah-media/components';

const params = new URLSearchParams(window.location.search);
// Prefer a real fMP4 fixture so the main-thread MSE session can play without
// FLV transmux. Override with ?src= for live/other streams.
const src = params.get('src') ?? '/fixtures/media/h264-http-fmp4/clip.mp4';

const app = document.getElementById('app');
if (app) {
  app.textContent = '';
  const player = document.createElement('cheetah-player');
  player.setAttribute('controls', '');
  player.setAttribute('src', src);
  player.setAttribute('live', '');
  player.setAttribute('volume', '0.8');
  player.setAttribute('worker-url', '/worker.js');
  player.setAttribute('wasm-url', '/wasm/cheetah_media_web_bindings.js');
  app.appendChild(player);
  console.log('[web-demo] <cheetah-player> demo mounted');
}
