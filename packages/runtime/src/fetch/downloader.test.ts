import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { BlobSink, StreamDownloader, type DownloadOptions } from './downloader';

function makeStream(chunks: Uint8Array[], closeWhenDone = true) {
  let index = 0;
  return new ReadableStream<Uint8Array>({
    pull(controller) {
      if (index >= chunks.length) {
        if (closeWhenDone) {
          controller.close();
        }
        return;
      }
      if (controller.desiredSize !== null && controller.desiredSize <= 0) {
        return;
      }
      const chunk = chunks[index];
      if (chunk) {
        controller.enqueue(chunk);
        index += 1;
      }
    },
  });
}

function makeOptions(url: string, sink?: { write: (chunk: Uint8Array) => void; close: () => void }): DownloadOptions {
  return {
    url,
    sink: sink ?? new BlobSink(),
  };
}

describe('StreamDownloader', () => {
  beforeEach(() => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async (_url: string | URL | Request, init?: RequestInit) => {
        const url = typeof _url === 'string' ? _url : _url.toString();
        if (url.includes('fail')) {
          return new Response('error', { status: 500 });
        }
        if (url.includes('range')) {
          const headers = init?.headers ? new Headers(init.headers) : new Headers();
          const range = headers.get('Range');
          if (range) {
            const start = Number.parseInt(range.replace('bytes=', ''), 10);
            return new Response(makeStream([new Uint8Array([start + 1, start + 2])]), { status: 206 });
          }
          // First request yields one chunk and then stalls so pause/resume can be tested.
          return new Response(makeStream([new Uint8Array([1, 2])], false));
        }
        return new Response(makeStream([new Uint8Array([1, 2]), new Uint8Array([3, 4, 5])]));
      }),
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('downloads a stream into a BlobSink', async () => {
    const sink = new BlobSink();
    const dl = new StreamDownloader();
    const result = await dl.start(makeOptions('https://example.com/video.mp4', sink));
    expect(result.bytesWritten).toBe(5);
    expect(sink.getBlob().size).toBe(5);
  });

  it('rejects invalid URLs', async () => {
    const dl = new StreamDownloader();
    await expect(dl.start(makeOptions('not-a-url'))).rejects.toMatchObject({ code: 7001 });
  });

  it('reports HTTP errors', async () => {
    const dl = new StreamDownloader();
    await expect(dl.start(makeOptions('https://example.com/fail'))).rejects.toMatchObject({ code: 7004 });
    expect(dl.progress.state).toBe('error');
  });

  it('applies a transform and skips empty results', async () => {
    const sink = new BlobSink();
    const dl = new StreamDownloader();
    await dl.start({
      ...makeOptions('https://example.com/video.mp4'),
      sink,
      transform: (chunk) => (chunk.length > 2 ? chunk : undefined),
    });
    expect(sink.getBlob().size).toBe(3);
  });

  it('calls onProgress for each chunk', async () => {
    const onProgress = vi.fn();
    const dl = new StreamDownloader();
    await dl.start({
      ...makeOptions('https://example.com/video.mp4'),
      onProgress,
    });
    expect(onProgress).toHaveBeenCalledTimes(2);
    expect(onProgress).toHaveBeenLastCalledWith(expect.objectContaining({ bytesWritten: 5 }));
  });

  it('resumes from the last received byte with Range header', async () => {
    const sink = new BlobSink();
    const dl = new StreamDownloader();
    const startPromise = dl.start({
      ...makeOptions('https://example.com/range'),
      sink,
      onProgress: (p) => {
        if (p.bytesWritten >= 2) dl.pause();
      },
    });
    const startResult = await startPromise;
    expect(startResult.bytesWritten).toBe(2);
    expect(dl.progress.state).toBe('paused');
    expect(sink.getBlob().size).toBe(2);

    const result = await dl.resume({ ...makeOptions('https://example.com/range'), sink });
    expect(result.bytesWritten).toBe(4);
    expect(sink.getBlob().size).toBe(4);
  });

  it('resumes using raw received bytes when transform changes chunk size', async () => {
    const sink = new BlobSink();
    const requestedRanges: (string | null)[] = [];
    const dl = new StreamDownloader();

    function makeResumableStream(chunks: Uint8Array[], signal: AbortSignal | undefined, closeOnDone = true) {
      let index = 0;
      return new ReadableStream<Uint8Array>({
        start(controller) {
          if (signal) {
            signal.addEventListener('abort', () => {
              try {
                controller.close();
              } catch {
                // already closed
              }
            }, { once: true });
          }
        },
        pull(controller) {
          if (index >= chunks.length) {
            if (closeOnDone) controller.close();
            return;
          }
          if (controller.desiredSize !== null && controller.desiredSize <= 0) return;
          const chunk = chunks[index];
          if (chunk) {
            controller.enqueue(chunk);
            index += 1;
          }
        },
      });
    }

    vi.stubGlobal(
      'fetch',
      vi.fn(async (_url: string | URL | Request, init?: RequestInit) => {
        const headers = init?.headers ? new Headers(init.headers) : new Headers();
        const range = headers.get('Range');
        const signal = init?.signal ?? undefined;
        requestedRanges.push(range);
        if (range) {
          const start = Number.parseInt(range.replace('bytes=', ''), 10);
          return new Response(makeResumableStream([new Uint8Array([start + 1, start + 2])], signal), { status: 206 });
        }
        return new Response(makeResumableStream([new Uint8Array([1, 2, 3, 4])], signal, false));
      }),
    );

    const startPromise = dl.start({
      url: 'https://example.com/range',
      sink,
      transform: (chunk) => {
        // Drop the second half of every 4-byte block, halving the output size.
        return chunk.length >= 2 ? chunk.subarray(0, 2) : chunk;
      },
      onProgress: (p) => {
        if (p.bytesWritten >= 2) dl.pause();
      },
    });
    const startResult = await startPromise;
    expect(startResult.bytesWritten).toBe(2);

    const result = await dl.resume({ url: 'https://example.com/range', sink });
    expect(result.bytesWritten).toBe(4);
    expect(requestedRanges[1]).toBe('bytes=4-');
  });

  it('stop aborts a running download', async () => {
    const dl = new StreamDownloader();
    const start = dl.start({
      ...makeOptions('https://example.com/video.mp4'),
      onProgress: (p) => {
        if (p.bytesWritten >= 2) dl.stop();
      },
    });
    await expect(start).rejects.toMatchObject({ code: 7006 });
    expect(dl.progress.state).toBe('idle');
  });

  it('stop closes the sink when the download is paused', async () => {
    const close = vi.fn();
    const sink = { write: () => undefined, close };
    const dl = new StreamDownloader();
    const start = dl.start({
      ...makeOptions('https://example.com/range'),
      sink,
      onProgress: (p) => {
        if (p.bytesWritten >= 2) dl.pause();
      },
    });
    await start;
    expect(close).not.toHaveBeenCalled();
    await dl.stop();
    expect(close).toHaveBeenCalledTimes(1);
  });

  it('calling stop before start is a no-op', async () => {
    const dl = new StreamDownloader();
    await expect(dl.stop()).resolves.toBeUndefined();
  });

  it('closes the sink when the download completes', async () => {
    const close = vi.fn();
    const sink = { write: () => undefined, close };
    const dl = new StreamDownloader();
    await dl.start(makeOptions('https://example.com/video.mp4', sink));
    expect(close).toHaveBeenCalledTimes(1);
  });
});
