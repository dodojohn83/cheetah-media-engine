import { createPlayer, Player } from '@cheetah-media/web';

export { createPlayer };
export type { Player };

export interface PlayerComponent {
  player: Player;
  mount(container: HTMLElement): void;
}

export function createPlayerComponent(): PlayerComponent {
  const player = createPlayer();
  return {
    player,
    mount: (_container: HTMLElement) => { /* TODO */ },
  };
}
