import { createPlayer, type CheetahPlayer, type PlayerConfig } from '@cheetah-media/web';
import { CheetahPlayerElement } from './player-element';

export { createPlayer, CheetahPlayerElement };
export type { CheetahPlayer, PlayerConfig };

export interface PlayerComponentOptions extends PlayerConfig {
  workerUrl?: string;
  wasmUrl?: string;
}

export interface PlayerComponent {
  player: CheetahPlayer;
  mount(container: HTMLElement): void;
  unmount(): void;
}

export function createPlayerComponent(options: PlayerComponentOptions = {}): PlayerComponent {
  const { workerUrl, wasmUrl, ...rest } = options;
  const runtimeConfig = {
    ...rest.runtime,
    ...(workerUrl !== undefined ? { workerUrl } : {}),
    ...(wasmUrl !== undefined ? { wasmUrl } : {}),
  };
  const player = createPlayer({
    ...rest,
    runtime: runtimeConfig,
  });
  let container: HTMLElement | undefined;
  let video: HTMLVideoElement | undefined;

  return {
    player,
    mount(parent: HTMLElement): void {
      if (video && video.parentNode) {
        video.parentNode.removeChild(video);
      }
      container = parent;
      video = document.createElement('video');
      video.autoplay = false;
      video.playsInline = true;
      video.style.width = '100%';
      video.style.height = '100%';
      parent.appendChild(video);
    },
    unmount(): void {
      if (video && container) {
        container.removeChild(video);
        video = undefined;
      }
      container = undefined;
      void player.destroy();
    },
  };
}

if (typeof customElements !== 'undefined' && !customElements.get('cheetah-player')) {
  customElements.define('cheetah-player', CheetahPlayerElement);
}
