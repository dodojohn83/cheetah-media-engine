import { createPlayer, type Player } from '@cheetah-media/web';

export { createPlayer };
export type { Player };

export interface PlayerComponent {
  player: Player;
  mount(container: HTMLElement): void;
  unmount(): void;
}

export interface PlayerComponentOptions {
  workerUrl?: string;
  wasmUrl?: string;
}

export function createPlayerComponent(options: PlayerComponentOptions = {}): PlayerComponent {
  const player = createPlayer(options);
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
