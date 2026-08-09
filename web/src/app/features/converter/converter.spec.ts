import { ComponentFixture, TestBed } from '@angular/core/testing';

import { WasmConverter } from '../../core/wasm-converter';
import { Converter } from './converter';

describe('Converter', () => {
  let component: Converter;
  let fixture: ComponentFixture<Converter>;
  const convert = vi.fn();

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [Converter],
      providers: [{ provide: WasmConverter, useValue: { convert } }],
    }).compileComponents();

    fixture = TestBed.createComponent(Converter);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });

  it('accepts an RTF file and enables local conversion', async () => {
    const input = fixture.nativeElement.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File(['{\\rtf1 hello}'], 'hello.rtf', { type: 'application/rtf' });
    Object.defineProperty(input, 'files', { value: [file] });

    input.dispatchEvent(new Event('change'));
    await fixture.whenStable();

    const button = fixture.nativeElement.querySelector('.primary') as HTMLButtonElement;
    expect(fixture.nativeElement.textContent).toContain('hello.rtf');
    expect(button.disabled).toBe(false);
  });

  it('converts the selected file through the WASM worker service', async () => {
    const input = fixture.nativeElement.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File(['{\\rtf1 hello}'], 'hello.rtf', { type: 'application/rtf' });
    Object.defineProperty(file, 'arrayBuffer', {
      value: () => Promise.resolve(new Uint8Array([1, 2, 3]).buffer),
    });
    Object.defineProperty(input, 'files', { value: [file] });
    convert.mockResolvedValue(new Uint8Array([37, 80, 68, 70]));

    input.dispatchEvent(new Event('change'));
    await fixture.whenStable();
    (fixture.nativeElement.querySelector('.primary') as HTMLButtonElement).click();
    await fixture.whenStable();

    expect(convert).toHaveBeenCalledOnce();
    expect(fixture.nativeElement.textContent).toContain('Download PDF');
  });
});
