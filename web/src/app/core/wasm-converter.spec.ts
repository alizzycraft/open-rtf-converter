import { TestBed } from '@angular/core/testing';

import { WasmConverter } from './wasm-converter';

describe('WasmConverter', () => {
  let service: WasmConverter;
  let worker: MockWorker;

  beforeEach(() => {
    vi.stubGlobal('Worker', MockWorker);
    TestBed.configureTestingModule({});
    service = TestBed.inject(WasmConverter);
    worker = MockWorker.instance;
  });

  afterEach(() => vi.unstubAllGlobals());

  it('should be created', () => {
    expect(service).toBeTruthy();
  });

  it('returns converted PDF bytes from the worker', async () => {
    const result = service.convert(new Uint8Array([1, 2, 3]));

    worker.emit('message', {
      data: { id: 1, ok: true, pdf: new Uint8Array([37, 80, 68, 70]).buffer },
    });

    await expect(result).resolves.toEqual(new Uint8Array([37, 80, 68, 70]));
    expect(worker.postMessage).toHaveBeenCalledOnce();
  });

  it('rejects conversion errors returned by the worker', async () => {
    const result = service.convert(new Uint8Array([1]));

    worker.emit('message', { data: { id: 1, ok: false, error: 'Invalid RTF' } });

    await expect(result).rejects.toThrow('Invalid RTF');
  });
});

class MockWorker {
  static instance: MockWorker;
  readonly postMessage = vi.fn();
  private readonly listeners = new Map<string, (event: MessageEvent) => void>();

  constructor() {
    MockWorker.instance = this;
  }

  addEventListener(type: string, listener: (event: MessageEvent) => void): void {
    this.listeners.set(type, listener);
  }

  emit(type: string, event: MessageEventInit): void {
    this.listeners.get(type)?.(new MessageEvent(type, event));
  }
}
