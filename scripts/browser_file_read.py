"""Capture bounded, format-neutral browser file-read failure evidence."""
from __future__ import annotations

from browser_protocol import BrowserRuntimeError, WebDriverClient
from browser_trace import Trace


MAX_READ_DIAGNOSTIC_BYTES = 1024 * 1024
READ_DIAGNOSTIC_TIMEOUT_MS = 5_000

READ_FAILURE_DIAGNOSTIC = """
const done = arguments[arguments.length - 1];
const maxBytes = __MAX_READ_DIAGNOSTIC_BYTES__;
const timeoutMs = __READ_DIAGNOSTIC_TIMEOUT_MS__;
const file = window.metisSelectedFile;
if (!(file instanceof File)) {
  done({status: 'unavailable', reason: 'no-selected-file'});
  return;
}
const exceptionName = error =>
  error && typeof error.name === 'string' && error.name ? error.name : 'Error';
(async () => {
const cleanupExceptions = [];
const probe = (read, onTimeout = () => {}) => new Promise(resolve => {
  let settled = false;
  const finish = result => {
    if (settled) return;
    settled = true;
    clearTimeout(timer);
    resolve(result);
  };
  const timer = setTimeout(() => {
    if (settled) return;
    settled = true;
    clearTimeout(timer);
    Promise.resolve().then(onTimeout).then(
      () => resolve({exception: 'TimeoutError'}),
      error => resolve({
        exception: 'TimeoutError', cleanup_exception: exceptionName(error)
      })
    );
  }, timeoutMs);
  Promise.resolve().then(read).then(async buffer => {
    if (!(buffer instanceof ArrayBuffer)) throw new DOMException('', 'DataError');
    if (buffer.byteLength > maxBytes) throw new DOMException('', 'QuotaExceededError');
    const digest = await crypto.subtle.digest('SHA-256', buffer);
    finish({
      bytes: buffer.byteLength,
      sha256: Array.from(
        new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')
      ).join('')
    });
  }, error => finish({exception: exceptionName(error)})).catch(
    error => finish({exception: exceptionName(error)})
  );
});
const bounded = file.slice(0, Math.min(file.size, maxBytes));
const original = await probe(() => {
  if (file.size > maxBytes) throw new DOMException('', 'QuotaExceededError');
  return file.arrayBuffer();
});
const sliceArrayBuffer = await probe(() => bounded.arrayBuffer());
let reader = null;
let removeReaderListeners = () => {};
const sliceFileReader = await probe(() => new Promise((resolve, reject) => {
  reader = new FileReader();
  const loaded = () => {
    const result = reader.result;
    removeReaderListeners();
    resolve(result);
  };
  const failed = () => {
    const error = reader.error || new DOMException('', 'NotReadableError');
    removeReaderListeners();
    reject(error);
  };
  const aborted = () => {
    removeReaderListeners();
    reject(new DOMException('', 'AbortError'));
  };
  removeReaderListeners = () => {
    reader.removeEventListener('load', loaded);
    reader.removeEventListener('error', failed);
    reader.removeEventListener('abort', aborted);
    removeReaderListeners = () => {};
  };
  reader.addEventListener('load', loaded);
  reader.addEventListener('error', failed);
  reader.addEventListener('abort', aborted);
  try {
    reader.readAsArrayBuffer(bounded);
  } catch (error) {
    removeReaderListeners();
    reject(error);
  }
}), () => {
  if (reader && reader.readyState === FileReader.LOADING) reader.abort();
  removeReaderListeners();
});
let objectUrl = null;
let controller = null;
let streamReader = null;
let fetchCleanupPromise = null;
const cleanupTimeoutMs = 1000;
const recordCleanupException = (operation, error) => {
  cleanupExceptions.push({operation, exception: exceptionName(error)});
};
const releaseStreamReader = readerToRelease => new Promise(resolve => {
  let settled = false;
  const finish = cancellationError => {
    if (settled) return;
    settled = true;
    clearTimeout(timer);
    if (cancellationError) {
      recordCleanupException('stream-cancel', cancellationError);
    }
    try {
      readerToRelease.releaseLock();
    } catch (error) {
      recordCleanupException('stream-release-lock', error);
    }
    resolve();
  };
  const timer = setTimeout(
    () => finish(new DOMException('', 'TimeoutError')), cleanupTimeoutMs
  );
  try {
    Promise.resolve(readerToRelease.cancel()).then(
      () => finish(null), error => finish(error)
    );
  } catch (error) {
    finish(error);
  }
});
const releaseFetch = () => {
  if (fetchCleanupPromise) return fetchCleanupPromise;
  fetchCleanupPromise = (async () => {
    const readerToRelease = streamReader;
    const controllerToAbort = controller;
    const urlToRevoke = objectUrl;
    streamReader = null;
    controller = null;
    objectUrl = null;
    if (controllerToAbort) {
      try {
        controllerToAbort.abort();
      } catch (error) {
        recordCleanupException('fetch-abort', error);
      }
    }
    if (urlToRevoke) {
      try {
        URL.revokeObjectURL(urlToRevoke);
      } catch (error) {
        recordCleanupException('object-url-revoke', error);
      }
    }
    if (readerToRelease) await releaseStreamReader(readerToRelease);
  })();
  return fetchCleanupPromise;
};
const blobUrlStream = await probe(async () => {
  objectUrl = URL.createObjectURL(bounded);
  controller = new AbortController();
  try {
    const response = await fetch(objectUrl, {signal: controller.signal});
    if (!response.ok) throw new DOMException('', 'NetworkError');
    if (!response.body) throw new DOMException('', 'NotSupportedError');
    streamReader = response.body.getReader();
    const chunks = [];
    let bytes = 0;
    while (true) {
      const part = await streamReader.read();
      if (part.done) break;
      bytes += part.value.byteLength;
      if (bytes > maxBytes) throw new DOMException('', 'QuotaExceededError');
      chunks.push(part.value);
    }
    const merged = new Uint8Array(bytes);
    let offset = 0;
    for (const chunk of chunks) {
      merged.set(chunk, offset);
      offset += chunk.byteLength;
    }
    return merged.buffer;
  } finally {
    await releaseFetch();
  }
}, releaseFetch);
return {
  status: 'captured',
  selected_bytes: file.size,
  bounded_bytes: bounded.size,
  cleanup_exceptions: cleanupExceptions,
  probes: {
    original_file_array_buffer: original,
    bounded_slice_array_buffer: sliceArrayBuffer,
    bounded_slice_file_reader: sliceFileReader,
    bounded_blob_url_stream: blobUrlStream
  }
};
})().then(done, error => done({status: 'failed', exception: exceptionName(error)}));
"""
READ_FAILURE_DIAGNOSTIC = (
    READ_FAILURE_DIAGNOSTIC.replace(
        "__MAX_READ_DIAGNOSTIC_BYTES__", str(MAX_READ_DIAGNOSTIC_BYTES)
    ).replace("__READ_DIAGNOSTIC_TIMEOUT_MS__", str(READ_DIAGNOSTIC_TIMEOUT_MS))
)


def capture_file_read_diagnostic(client: WebDriverClient, trace: Trace) -> None:
    """Append bounded read-path evidence without exposing selected-file details."""
    try:
        diagnostic = client.execute_async(READ_FAILURE_DIAGNOSTIC)
    except BrowserRuntimeError:
        diagnostic = {"status": "unavailable", "reason": "webdriver-error"}
    if not isinstance(diagnostic, dict):
        diagnostic = {"status": "unavailable", "reason": "invalid-result"}
    trace.actions.append({"file_read_diagnostic": diagnostic})
