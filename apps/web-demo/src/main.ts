import '@cheetah-media/components';

const params = new URLSearchParams(window.location.search);
const pathname = window.location.pathname;
// The isolated page needs a real fixture so the MSE session reaches preroll.
// The root demo intentionally uses an unavailable source so the player surfaces
// the failure-state UI by default; override with ?src= to test real streams.
const defaultSrc =
  pathname === '/isolated' || pathname === '/isolated/'
    ? '/fixtures/media/h264-http-fmp4/clip.mp4'
    : 'test.flv';
const src = params.get('src') ?? defaultSrc;

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
