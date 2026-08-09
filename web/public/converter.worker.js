let bindingsPromise;

function loadBindings() {
  bindingsPromise ??= import('./wasm/open_rtf_converter.js').then(async (bindings) => {
    await bindings.default();
    return bindings;
  });
  return bindingsPromise;
}

self.addEventListener('message', async ({ data }) => {
  const { id, bytes } = data;
  try {
    const bindings = await loadBindings();
    const pdf = bindings.convert_rtf_to_pdf_browser(new Uint8Array(bytes));
    const output = pdf.slice().buffer;
    self.postMessage({ id, ok: true, pdf: output }, [output]);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    self.postMessage({ id, ok: false, error: message || 'Local conversion failed.' });
  }
});
