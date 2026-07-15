export type MessageKey =
  | 'play'
  | 'pause'
  | 'loading'
  | 'preroll'
  | 'rebuffering'
  | 'failed'
  | 'error'
  | 'retry'
  | 'snapshot'
  | 'recordStart'
  | 'recordStop'
  | 'fullscreen'
  | 'mute'
  | 'unmute'
  | 'volume'
  | 'autoplayBlocked'
  | 'unsupported'
  | 'latencyStatus'
  | 'buffered'
  | 'controls'
  | 'idle'
  | 'playing'
  | 'paused'
  | 'stopping'
  | 'destroyed';

const messages: Record<string, Record<MessageKey, string>> = {
  en: {
    play: 'Play',
    pause: 'Pause',
    loading: 'Loading…',
    preroll: 'Buffering…',
    rebuffering: 'Rebuffering…',
    failed: 'Playback failed',
    error: 'Error',
    retry: 'Retry',
    snapshot: 'Snapshot',
    recordStart: 'Start recording',
    recordStop: 'Stop recording',
    fullscreen: 'Fullscreen',
    mute: 'Mute',
    unmute: 'Unmute',
    volume: 'Volume',
    autoplayBlocked: 'Click to start playback',
    unsupported: 'Unsupported configuration',
    latencyStatus: 'Latency',
    buffered: 'Buffered',
    controls: 'Player controls',
    idle: 'Idle',
    playing: 'Playing',
    paused: 'Paused',
    stopping: 'Stopping',
    destroyed: 'Destroyed',
  },
  zh: {
    play: '播放',
    pause: '暂停',
    loading: '加载中…',
    preroll: '缓冲中…',
    rebuffering: '重新缓冲中…',
    failed: '播放失败',
    error: '错误',
    retry: '重试',
    snapshot: '截图',
    recordStart: '开始录制',
    recordStop: '停止录制',
    fullscreen: '全屏',
    mute: '静音',
    unmute: '取消静音',
    volume: '音量',
    autoplayBlocked: '点击开始播放',
    unsupported: '不支持的配置',
    latencyStatus: '延迟',
    buffered: '缓冲',
    controls: '播放器控件',
    idle: '空闲',
    playing: '播放中',
    paused: '已暂停',
    stopping: '停止中',
    destroyed: '已销毁',
  },
};

export function detectLocale(): string {
  if (typeof document !== 'undefined' && document.documentElement?.lang) {
    const lang = document.documentElement.lang.toLowerCase();
    if (lang.startsWith('zh')) return 'zh';
  }
  if (typeof navigator !== 'undefined' && navigator.language) {
    const lang = navigator.language.toLowerCase();
    if (lang.startsWith('zh')) return 'zh';
  }
  return 'en';
}

const defaultMessages = messages.en!;

export function getMessage(locale: string, key: MessageKey): string {
  const table = messages[locale] ?? defaultMessages;
  return (table ?? defaultMessages)[key] ?? defaultMessages[key] ?? key;
}
