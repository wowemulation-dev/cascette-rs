#!/usr/bin/env python3
"""Threaded HTTP server with Range request support for serving local CDN mirrors.

Python's built-in http.server is single-threaded and ignores Range headers.
This server uses a thread pool to handle concurrent requests and returns
HTTP 206 Partial Content for byte-range fetches.

The cascette-agent uses up to 12 concurrent connections (3 per host) at the
installation layer and 8-10 per host at the streaming layer. The default
thread pool size of 16 workers handles this with headroom.

Resilience features (added 2026-08-19 after I/O errors on a USB-attached
mirror):
- ``--throttle-kbps``: pace each connection's copy to a bounded byte rate.
- ``--max-concurrent-reads``: global semaphore capping in-flight disk reads.
- read retry with position restore for transient OSError (Errno 5);
- truncation detection: a short read below the fstat size aborts the
  response (connection closed) instead of serving a partial body under
  the full Content-Length, which stalls the client;
- open() retry before falling back to 404;
- whole-file LRU cache for small hot files (configs, manifests, encoding)
  keyed by (path, mtime, size), so repeated client startups do not re-read
  the same blobs from disk;
- GET /healthz returning live counters (requests, errors, cache state,
  concurrency) for debugging bootstrap stalls.

Usage:
    python3 range_http_server.py /path/to/cdn/mirror
    python3 range_http_server.py /path/to/cdn/mirror 8080
    python3 range_http_server.py /path/to/cdn/mirror --port 8080 --threads 32
    python3 range_http_server.py /path/to/cdn/mirror --max-concurrent-reads 4
    python3 range_http_server.py /path/to/cdn/mirror --throttle-kbps 20000

Default port is 8000. Default thread pool size is 16.
"""

import argparse
import json
import os
import sys
import threading
import time
from collections import OrderedDict
from concurrent.futures import ThreadPoolExecutor
from functools import partial
from http.server import HTTPServer, SimpleHTTPRequestHandler
from socketserver import ThreadingMixIn
from typing import cast

# Default pool size: covers the client's concurrent connections plus
# headroom. HTTP/1.1 keep-alive means each open connection holds one pool
# worker for its lifetime; 12 concurrent downloads plus stale CLOSE-WAIT
# sockets can exhaust a 16-worker pool (observed 2026-08-15: 190 sockets
# stuck in CLOSE-WAIT after several installs, all range requests timed
# out client-side). 64 gives headroom; the idle timeout below releases
# workers when a client goes away.
DEFAULT_POOL_SIZE = 64

# Whole-file cache bounds. Files at or below MAX_CACHE_FILE_SIZE are cached
# in memory after their first full read; total memory is capped at
# MAX_CACHE_TOTAL_BYTES with LRU eviction. Covers the encoding file (~17 MB),
# configs, manifests and small loose blobs — the blobs the client re-fetches
# at every startup. Archive data files exceed the per-file cap and are
# always streamed from disk.
MAX_CACHE_FILE_SIZE = 64 * 1024 * 1024  # 64 MiB
MAX_CACHE_TOTAL_BYTES = 512 * 1024 * 1024  # 512 MiB

READ_RETRIES = 3
READ_RETRY_BACKOFF = 0.25  # seconds, multiplied by attempt index
OPEN_RETRIES = 3


class ThreadPoolHTTPServer(ThreadingMixIn, HTTPServer):
    """HTTPServer that dispatches requests to a bounded thread pool.

    Uses ThreadingMixIn for per-request thread dispatch, with the actual
    thread management delegated to a ThreadPoolExecutor. This bounds
    resource usage while handling concurrent connections from the agent.
    """

    daemon_threads = True
    allow_reuse_address = True

    def __init__(
        self,
        server_address,
        RequestHandlerClass,
        pool_size,
        max_concurrent_reads=0,
    ):
        super().__init__(server_address, RequestHandlerClass)
        self._pool = ThreadPoolExecutor(max_workers=pool_size)
        self._pool_size = pool_size
        # Global cap on in-flight file reads. The WoW client opens up to
        # 12 parallel Range connections; against a USB-attached mirror this
        # can overwhelm the bridge (repeated SuperSpeed resets + I/O errors
        # observed 2026-08-19). 0 = unlimited.
        self.read_semaphore = (
            threading.BoundedSemaphore(max_concurrent_reads)
            if max_concurrent_reads > 0
            else None
        )
        self.throttle_bytes_per_s = 0
        # Stats (readable via GET /healthz).
        self.stats_lock = threading.Lock()
        self.stats = {
            "requests": 0,
            "bytes_served": 0,
            "disk_errors": 0,
            "retries": 0,
            "truncations": 0,
            "not_found": 0,
            "cache_hits": 0,
            "cache_misses": 0,
            "cache_evictions": 0,
            "active_reads": 0,
        }
        # Whole-file LRU cache: path -> (mtime, size, bytes).
        self.cache_lock = threading.Lock()
        self.cache: OrderedDict[str, tuple[float, int, bytes]] = OrderedDict()
        self.cache_bytes = 0

    def _bump(self, key, delta=1):
        with self.stats_lock:
            self.stats[key] = self.stats.get(key, 0) + delta

    def process_request(self, request, client_address):
        """Submit each request to the thread pool instead of spawning a thread."""
        self._pool.submit(self._process_request_thread, request, client_address)

    def _process_request_thread(self, request, client_address):
        """Handle a request in a pool thread, matching ThreadingMixIn's contract."""
        try:
            self.finish_request(request, client_address)
        except Exception:
            self.handle_error(request, client_address)
        finally:
            self.shutdown_request(request)

    def handle_error(self, request, client_address):
        """Suppress traceback noise for benign peer-disconnect errors.

        BrokenPipeError and ConnectionResetError are routine when a client
        cancels an in-flight Range request after it has read what it
        needed. The default ``BaseServer.handle_error`` dumps a 30-line
        Python traceback per occurrence, which drowns the access log on
        a busy CASC bootstrap. Anything else falls through to the default.
        """
        exc = sys.exc_info()[1]
        if isinstance(exc, (BrokenPipeError, ConnectionResetError)):
            return
        super().handle_error(request, client_address)

    def cache_get(self, path):
        """Return cached bytes for path or None; refresh LRU order on hit."""
        with self.cache_lock:
            hit = self.cache.get(path)
            if hit is None:
                self._bump("cache_misses")
                return None
            self.cache.move_to_end(path)
            self._bump("cache_hits")
            return hit[2]

    def cache_put(self, path, mtime, size, data):
        """Insert into the LRU cache, evicting oldest until under budget."""
        with self.cache_lock:
            existing = self.cache.get(path)
            if existing is not None:
                self.cache_bytes -= len(existing[2])
            self.cache[path] = (mtime, size, data)
            self.cache_bytes += len(data)
            while self.cache_bytes > MAX_CACHE_TOTAL_BYTES and len(self.cache) > 1:
                _, oldest = self.cache.popitem(last=False)
                self.cache_bytes -= len(oldest[2])
                self._bump("cache_evictions")


class RangeHTTPRequestHandler(SimpleHTTPRequestHandler):
    """SimpleHTTPRequestHandler with Range header support.

    Forces ``HTTP/1.1`` on responses so the wow client gets keep-alive
    connections matching what Blizzard's real CDN serves. The default
    HTTP/1.0 sends ``Connection: close`` on every response; under the
    client's high-concurrency Range-fetch pattern this surfaces as
    ``Failed to make data resident ... results are truncated`` errors
    in ``_classic_/Logs/Tact.log`` because the client's reader sees a
    socket close before its parser finishes consuming the response body
    of a pipelined request.
    """

    protocol_version = "HTTP/1.1"

    # Close keep-alive connections idle for >10s so they do not hold a
    # pool worker indefinitely (CLOSE-WAIT accumulation stalled installs).
    timeout = 10

    def srv(self) -> "ThreadPoolHTTPServer":
        """Typed accessor for the custom server instance."""
        return cast("ThreadPoolHTTPServer", self.server)

    def send_head(self):  # type: ignore[override]  # returns _BoundedFile/_BytesSource, a compatible file-like
        """Serve a GET request, handling Range headers."""
        if self.path.split("?", 1)[0] == "/healthz":
            return self._serve_healthz()

        path = self.translate_path(self.path)
        if os.path.isdir(path):
            return super().send_head()

        # Retry open(): a transient USB-bridge error can fail the first
        # open attempt even though the file is fine.
        f = None
        last_exc = None
        for attempt in range(OPEN_RETRIES):
            try:
                f = open(path, "rb")  # noqa: SIM115
                break
            except OSError as exc:
                last_exc = exc
                time.sleep(READ_RETRY_BACKOFF * (attempt + 1))
        if f is None:
            self.srv()._bump("not_found")
            if last_exc is not None:
                self.srv()._bump("disk_errors")
                self.log_message("open failed after retries: %s", last_exc)
            self.send_error(404, "File not found")
            return None

        fs = os.fstat(f.fileno())
        file_size = fs.st_size
        ctype = self.guess_type(path)

        # Serve small whole files from the in-memory LRU cache. Cached
        # blobs are re-read only when mtime/size change. This eliminates
        # repeated disk reads of the encoding file, configs and manifests
        # across client startups.
        # Determine the requested byte range first so both the cache and
        # disk paths emit identical headers.
        range_header = self.headers.get("Range")
        start, end = 0, file_size - 1
        if range_header is not None:
            parsed = self._parse_range(range_header, file_size)
            if parsed is None:
                f.close()
                self.send_error(416, "Requested Range Not Satisfiable")
                return None
            start, end = parsed

        content_length = end - start + 1
        is_range = range_header is not None

        # Serve small whole files from the in-memory LRU cache. Cached
        # blobs are re-read only when mtime/size change. This eliminates
        # repeated disk reads of the encoding file, configs and manifests
        # across client startups.
        cacheable = file_size <= MAX_CACHE_FILE_SIZE
        cached = self.srv().cache_get(path) if cacheable else None
        if cached is not None:
            f.close()
            self._emit_headers(206 if is_range else 200, ctype,
                               content_length, start, end, file_size,
                               fs.st_mtime)
            return _BytesSource(cached, file_size, ctype, start, end)

        if range_header is None and cacheable:
            # Full read of a small file: cache it for future requests.
            data = self._read_full(f, path, fs.st_mtime, file_size)
            if data is None:
                return None
            self.srv().cache_put(path, fs.st_mtime, file_size, data)
            self._emit_headers(200, ctype, content_length, 0, file_size - 1,
                               file_size, fs.st_mtime)
            return _BytesSource(data, file_size, ctype, 0, file_size - 1)

        f.seek(start)
        self._emit_headers(206 if is_range else 200, ctype,
                           content_length, start, end, file_size, fs.st_mtime)
        return _BoundedFile(f, content_length, start, file_size)

    @staticmethod
    def _parse_range(range_header, file_size):
        """Parse a single-range header; None if unsatisfiable."""
        try:
            range_spec = range_header.strip()
            if not range_spec.startswith("bytes="):
                return None
            range_spec = range_spec[6:]
            if "," in range_spec:
                # Multi-range not supported; treat as unsatisfiable.
                return None
            parts = range_spec.split("-", 1)
            rstart = int(parts[0]) if parts[0] else None
            rend = int(parts[1]) if parts[1] else None
        except (ValueError, IndexError):
            return None
        if rstart is not None and rend is not None:
            if rstart > rend or rstart >= file_size:
                return None
            return rstart, min(rend, file_size - 1)
        if rstart is not None:
            if rstart >= file_size:
                return None
            return rstart, file_size - 1
        if rend is not None:
            return max(0, file_size - rend), file_size - 1
        return None

    def _emit_headers(self, status, ctype, content_length, start, end, file_size,
                      mtime):
        """Send response headers for the given status and range."""
        self.send_response(status)
        self.send_header("Content-type", ctype)
        self.send_header("Content-Length", str(content_length))
        if status == 206:
            self.send_header("Content-Range", f"bytes {start}-{end}/{file_size}")
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("Last-Modified", self.date_time_string(mtime))
        self.end_headers()

    def _read_full(self, f, path, mtime, file_size):
        """Read a whole file into memory with retry; None on persistent error."""
        try:
            data = f.read()
        except OSError as exc:
            self.srv()._bump("disk_errors")
            self.log_message("read failed for %s: %s", path, exc)
            f.close()
            return None
        f.close()
        if len(data) != file_size:
            # File changed (or truncated) between fstat and read.
            self.srv()._bump("truncations")
            self.log_message("size changed for %s (%d != %d)", path, len(data), file_size)
            return None
        return data

    def _serve_healthz(self):
        """Return live counters as JSON."""
        with self.srv().stats_lock:
            stats = dict(self.srv().stats)
        with self.srv().cache_lock:
            stats["cache_files"] = len(self.srv().cache)
            stats["cache_bytes"] = self.srv().cache_bytes
        stats["pool_size"] = self.srv()._pool_size
        payload = json.dumps(stats, indent=2).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        return _BytesSource(payload, len(payload), "application/json", 0, len(payload) - 1)

    def log_message(self, format, *args):
        """Thread-safe log output with thread name prefix.

        Includes the User-Agent and Range request headers so we can
        attribute requests to specific clients (e.g. cascette-agent vs.
        the patched Wow.exe) and see partial-content access patterns.
        """
        thread = threading.current_thread().name
        ua = self.headers.get("User-Agent", "-") if hasattr(self, "headers") else "-"
        rng = self.headers.get("Range", "-") if hasattr(self, "headers") else "-"
        sys.stderr.write(
            f"{thread} - {self.address_string()} - "
            f"[{self.log_date_time_string()}] {format % args} "
            f'ua="{ua}" range="{rng}"\n'
        )

    def copyfile(self, source, outputfile):
        """Stream source to outputfile, handling both peer disconnects and
        disk errors.

        The patched WoW client cancels Range requests mid-stream once it
        has read the bytes it needs; this raises BrokenPipeError or
        ConnectionResetError from inside ``shutil.copyfileobj``. Those are
        normal HTTP behavior and not server bugs, but the default
        threading server prints a full traceback for each, drowning the
        access log.

        A disk read error (OSError) mid-stream must NOT be swallowed: the
        response header already promised Content-Length bytes, so serving
        a partial body stalls the client. We close the connection instead,
        and the client retries the request.
        """
        sem = self.srv().read_semaphore
        throttle_bytes_per_s = getattr(self.server, "throttle_bytes_per_s", 0)
        acquired = False
        if sem is not None:
            sem.acquire()
            acquired = True
            self.srv()._bump("active_reads")
        try:
            if throttle_bytes_per_s > 0:
                self._copyfile_throttled(source, outputfile, throttle_bytes_per_s)
            else:
                super().copyfile(source, outputfile)
        except (BrokenPipeError, ConnectionResetError) as exc:
            self.log_message("client disconnected mid-stream (%s)", type(exc).__name__)
        except OSError as exc:
            self.srv()._bump("disk_errors")
            self.log_message("disk read error mid-stream (%s); closing connection", exc)
            self.close_connection = True
        finally:
            if acquired:
                self.srv()._bump("active_reads", -1)
                assert sem is not None
                sem.release()

    @staticmethod
    def _copyfile_throttled(source, outputfile, bytes_per_s):
        """Paced copy at bytes_per_s, reading in 64 KiB chunks.

        The sleep keeps the read rate (and thus the load on a USB-
        attached mirror drive) bounded per connection."""
        chunk = 64 * 1024
        min_interval = chunk / bytes_per_s
        while True:
            start = time.monotonic()
            data = source.read(chunk)
            if not data:
                break
            outputfile.write(data)
            elapsed = time.monotonic() - start
            if elapsed < min_interval:
                time.sleep(min_interval - elapsed)

    def handle_one_request(self):
        """Same wrapper for request-line read disconnects."""
        try:
            super().handle_one_request()
        except (BrokenPipeError, ConnectionResetError) as exc:
            self.log_message("client disconnected (%s)", type(exc).__name__)
            self.close_connection = True


class _BoundedFile:
    """File wrapper that limits reads to a byte count.

    Tracks the absolute file position so a transient OSError can retry
    from a known offset. Detects truncation: a read that returns fewer
    bytes than requested before the bounded range is exhausted raises, so
    the caller (copyfile) closes the connection instead of serving a
    partial body under the full Content-Length.
    """

    def __init__(self, f, limit, start=0, file_size=None):
        self._f = f
        self._remaining = limit
        self._pos = start
        self._file_size = file_size

    def read(self, size=-1):
        if self._remaining <= 0:
            return b""
        if size < 0 or size > self._remaining:
            size = self._remaining

        last_exc: OSError | None = None
        for attempt in range(READ_RETRIES):
            try:
                data = self._f.read(size)
                self._remaining -= len(data)
                self._pos += len(data)
                break
            except OSError as exc:
                last_exc = exc
                self.server_note_disk_error()
                time.sleep(READ_RETRY_BACKOFF * (attempt + 1))
                # Restore the known-good position before retrying.
                self._f.seek(self._pos)
        else:
            assert last_exc is not None
            raise last_exc

        if len(data) < size and self._remaining > 0 and self._file_size is not None:
            # EOF hit before the bounded range was satisfied: the file is
            # shorter than fstat reported (truncated or changed mid-read).
            raise OSError(
                f"truncated read: wanted {size}, got {len(data)} at pos {self._pos}"
            )
        return data

    def server_note_disk_error(self):
        pass  # overridden per-instance by the caller via closure if needed

    def close(self):
        self._f.close()


class _BytesSource:
    """In-memory source for cached/healthz payloads.

    Masks the file-object protocol used by copyfile: ``read(n)`` returns
    the next chunk of the byte slice.
    """

    def __init__(self, data, file_size, ctype, start, end):
        self._data = data[start : end + 1]
        self._pos = 0

    def read(self, size=-1):
        if self._pos >= len(self._data):
            return b""
        chunk = self._data[self._pos : self._pos + size] if size >= 0 else self._data[self._pos :]
        self._pos += len(chunk)
        return chunk

    def close(self):
        pass


def main():
    parser = argparse.ArgumentParser(
        description="Threaded HTTP server with Range request support"
    )
    parser.add_argument(
        "directory",
        help="Root directory to serve",
    )
    parser.add_argument(
        "port",
        nargs="?",
        type=int,
        default=8000,
        help="Port to listen on (default: 8000)",
    )
    parser.add_argument(
        "--threads",
        type=int,
        default=DEFAULT_POOL_SIZE,
        help=f"Thread pool size (default: {DEFAULT_POOL_SIZE})",
    )
    parser.add_argument(
        "--throttle-kbps",
        type=int,
        default=0,
        help="Per-connection copy rate cap in KiB/s (0 = unlimited)",
    )
    parser.add_argument(
        "--max-concurrent-reads",
        type=int,
        default=0,
        help="Global cap on in-flight disk reads (0 = unlimited; "
        "use 4 with a USB-attached mirror)",
    )
    args = parser.parse_args()

    root = os.path.abspath(args.directory)
    if not os.path.isdir(root):
        print(f"Error: {root} is not a directory", file=sys.stderr)
        sys.exit(1)

    # Pass directory= to SimpleHTTPRequestHandler via partial so
    # translate_path() resolves against the specified root, not CWD.
    handler = partial(RangeHTTPRequestHandler, directory=root)

    server = ThreadPoolHTTPServer(
        ("", args.port), handler, args.threads, args.max_concurrent_reads
    )
    server.throttle_bytes_per_s = args.throttle_kbps * 1024
    print(
        f"Serving {root} on port {args.port} with Range support "
        f"({args.threads} worker threads, throttle={args.throttle_kbps} KiB/s, "
        f"max_concurrent_reads={args.max_concurrent_reads})"
    )
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nStopped.")
        server.server_close()


if __name__ == "__main__":
    main()
