import { Service } from '@angular/core';

interface ConversionSuccess {
  readonly id: number;
  readonly ok: true;
  readonly pdf: ArrayBuffer;
}

interface ConversionFailure {
  readonly id: number;
  readonly ok: false;
  readonly error: string;
}

type ConversionResponse = ConversionSuccess | ConversionFailure;

interface PendingConversion {
  readonly resolve: (pdf: Uint8Array) => void;
  readonly reject: (error: Error) => void;
}

@Service()
export class WasmConverter {
  private readonly worker = new Worker(new URL('converter.worker.js', document.baseURI), {
    type: 'module',
    name: 'open-rtf-converter',
  });
  private readonly pending = new Map<number, PendingConversion>();
  private nextId = 1;
  private workerFailure: Error | null = null;

  constructor() {
    this.worker.addEventListener('message', ({ data }: MessageEvent<ConversionResponse>) => {
      const request = this.pending.get(data.id);
      if (!request) {
        return;
      }

      this.pending.delete(data.id);
      if (data.ok) {
        request.resolve(new Uint8Array(data.pdf));
      } else {
        request.reject(new Error(data.error));
      }
    });

    this.worker.addEventListener('error', () => {
      const error = new Error('The local conversion worker could not be started.');
      this.workerFailure = error;
      for (const request of this.pending.values()) {
        request.reject(error);
      }
      this.pending.clear();
    });
  }

  convert(input: Uint8Array): Promise<Uint8Array> {
    if (this.workerFailure) {
      return Promise.reject(this.workerFailure);
    }

    const id = this.nextId++;
    const bytes = input.slice().buffer;

    return new Promise<Uint8Array>((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ id, bytes }, [bytes]);
    });
  }
}
