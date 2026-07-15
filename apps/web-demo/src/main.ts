import '@cheetah-media/components';

const app = document.getElementById('app');
if (app) {
  app.textContent = '';
  const player = document.createElement('cheetah-player');
  player.setAttribute('controls', '');
  player.setAttribute('src', 'test.flv');
  player.setAttribute('live', '');
  player.setAttribute('volume', '0.8');
  app.appendChild(player);
  console.log('[web-demo] <cheetah-player> demo mounted');
}
