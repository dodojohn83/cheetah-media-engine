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
  | 'destroyed'
  | 'ptzTitle'
  | 'ptzUp'
  | 'ptzDown'
  | 'ptzLeft'
  | 'ptzRight'
  | 'ptzZoomIn'
  | 'ptzZoomOut'
  | 'ptzPresetSet'
  | 'ptzPresetCall'
  | 'ptzPresetDelete'
  | 'ptzStop'
  | 'ptzPresetNumber'
  | 'ptzSpeed'
  | 'ptzUpLeft'
  | 'ptzUpRight'
  | 'ptzDownLeft'
  | 'ptzDownRight'
  | 'intercomStart'
  | 'intercomStop';

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
    ptzTitle: 'PTZ',
    ptzUp: 'Up',
    ptzDown: 'Down',
    ptzLeft: 'Left',
    ptzRight: 'Right',
    ptzUpLeft: 'Up left',
    ptzUpRight: 'Up right',
    ptzDownLeft: 'Down left',
    ptzDownRight: 'Down right',
    ptzZoomIn: 'Zoom in',
    ptzZoomOut: 'Zoom out',
    ptzPresetSet: 'Set preset',
    ptzPresetCall: 'Call preset',
    ptzPresetDelete: 'Delete preset',
    ptzStop: 'Stop',
    ptzPresetNumber: 'Preset number',
    ptzSpeed: 'Speed',
    intercomStart: 'Start intercom',
    intercomStop: 'Stop intercom',
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
    ptzTitle: '云台',
    ptzUp: '上',
    ptzDown: '下',
    ptzLeft: '左',
    ptzRight: '右',
    ptzUpLeft: '左上',
    ptzUpRight: '右上',
    ptzDownLeft: '左下',
    ptzDownRight: '右下',
    ptzZoomIn: '变焦放大',
    ptzZoomOut: '变焦缩小',
    ptzPresetSet: '设置预置位',
    ptzPresetCall: '调用预置位',
    ptzPresetDelete: '删除预置位',
    ptzStop: '停止',
    ptzPresetNumber: '预置位编号',
    ptzSpeed: '速度',
    intercomStart: '开始对讲',
    intercomStop: '停止对讲',
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
