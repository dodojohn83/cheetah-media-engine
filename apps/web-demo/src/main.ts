import { createPlayerComponent } from '@cheetah-media/components';

const app = document.getElementById('app');
if (app) {
  const component = createPlayerComponent();
  component.mount(app);
  app.textContent = 'Cheetah Media Engine Web Demo';
  console.log('[web-demo] Cheetah Media Engine demo mounted');
}
