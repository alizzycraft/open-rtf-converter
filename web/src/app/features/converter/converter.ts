import { Component, computed, inject, signal } from '@angular/core';

import { WasmConverter } from '../../core/wasm-converter';

type ConversionState = 'empty' | 'selected' | 'converting' | 'ready' | 'error';

const MAX_FILE_BYTES = 10 * 1024 * 1024;

@Component({
  selector: 'app-converter',
  imports: [],
  templateUrl: './converter.html',
  styleUrl: './converter.scss',
})
export class Converter {
  private readonly wasmConverter = inject(WasmConverter);

  protected readonly file = signal<File | null>(null);
  protected readonly state = signal<ConversionState>('empty');
  protected readonly isDragging = signal(false);
  protected readonly errorMessage = signal('');
  private readonly pdf = signal<Uint8Array | null>(null);

  protected readonly formattedSize = computed(() => formatBytes(this.file()?.size ?? 0));
  protected readonly statusMessage = computed(() => {
    switch (this.state()) {
      case 'selected':
        return 'Ready to convert locally.';
      case 'converting':
        return 'Converting in a private browser worker…';
      case 'ready':
        return `Your ${formatBytes(this.pdf()?.byteLength ?? 0)} PDF is ready.`;
      case 'error':
        return this.errorMessage();
      default:
        return 'Choose one .rtf file, up to 10 MB.';
    }
  });

  protected chooseFile(input: HTMLInputElement): void {
    if (this.state() === 'converting') {
      return;
    }
    input.click();
  }

  protected onFileInput(event: Event): void {
    const input = event.target as HTMLInputElement;
    this.acceptFile(input.files?.[0] ?? null);
    input.value = '';
  }

  protected onDrop(event: DragEvent): void {
    event.preventDefault();
    this.isDragging.set(false);
    if (this.state() === 'converting') {
      return;
    }
    this.acceptFile(event.dataTransfer?.files[0] ?? null);
  }

  protected onDragOver(event: DragEvent): void {
    event.preventDefault();
    if (this.state() !== 'converting') {
      this.isDragging.set(true);
    }
  }

  protected onDragLeave(event: DragEvent): void {
    if (!(event.currentTarget as HTMLElement).contains(event.relatedTarget as Node | null)) {
      this.isDragging.set(false);
    }
  }

  protected onDropzoneKeydown(event: KeyboardEvent, input: HTMLInputElement): void {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      this.chooseFile(input);
    }
  }

  protected clearFile(): void {
    if (this.state() === 'converting') {
      return;
    }
    this.file.set(null);
    this.pdf.set(null);
    this.errorMessage.set('');
    this.state.set('empty');
  }

  protected async convertOrDownload(): Promise<void> {
    const selectedFile = this.file();
    if (!selectedFile || this.state() === 'converting') {
      return;
    }
    if (this.state() === 'ready') {
      this.downloadPdf();
      return;
    }

    this.state.set('converting');
    this.errorMessage.set('');
    this.pdf.set(null);

    try {
      const input = new Uint8Array(await selectedFile.arrayBuffer());
      this.pdf.set(await this.wasmConverter.convert(input));
      this.state.set('ready');
    } catch (error: unknown) {
      this.errorMessage.set(readErrorMessage(error));
      this.state.set('error');
    }
  }

  private acceptFile(file: File | null): void {
    if (!file) {
      return;
    }
    if (!file.name.toLowerCase().endsWith('.rtf')) {
      this.rejectFile('Choose a file with the .rtf extension.');
      return;
    }
    if (file.size > MAX_FILE_BYTES) {
      this.rejectFile('This file is larger than the 10 MB browser limit.');
      return;
    }

    this.file.set(file);
    this.pdf.set(null);
    this.errorMessage.set('');
    this.state.set('selected');
  }

  private rejectFile(message: string): void {
    this.file.set(null);
    this.pdf.set(null);
    this.errorMessage.set(message);
    this.state.set('error');
  }

  private downloadPdf(): void {
    const pdf = this.pdf();
    if (!pdf) {
      return;
    }

    const fileName = (this.file()?.name ?? 'document').replace(/\.rtf$/i, '');
    const blob = new Blob([pdf.slice().buffer], { type: 'application/pdf' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `${fileName}.pdf`;
    anchor.click();
    window.setTimeout(() => URL.revokeObjectURL(url), 0);
  }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1_048_576) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / 1_048_576).toFixed(1)} MB`;
}

function readErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  if (typeof error === 'string' && error.trim()) {
    return error;
  }
  return 'The document could not be converted locally.';
}
