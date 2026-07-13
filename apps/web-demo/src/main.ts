import { createPlayerComponent } from '@cheetah-media/components';

const app = document.getElementById('app');
if (app) {
  app.textContent = 'Cheetah Media Engine Web Demo';
  const component = createPlayerComponent();
  component.mount(app);
  console.log('[web-demo] Cheetah Media Engine demo mounted');
}
