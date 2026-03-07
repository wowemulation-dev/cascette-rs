#!/usr/bin/env python3
"""Threaded HTTP server with Range request support for serving local CDN mirrors.

Python's built-in http.server is single-threaded and ignores Range headers.
This server uses a thread pool to handle concurrent requests and returns
HTTP 206 Partial Content for byte-range fetches.

The cascette-agent uses up to 12 concurrent connections (3 per host) at the
installation layer and 8-10 per host at the streaming layer. The default
thread pool size of 16 workers handles this with headroom.

Usage:
    python3 range_http_server.py /path/to/cdn/mirror
    python3 range_http_server.py /path/to/cdn/mirror 8080
    python3 range_http_server.py /path/to/cdn/mirror --port 8080 --threads 32

Default port is 8000. Default thread pool size is 16.
"""

import argparse
import os
import sys
import threading
from concurrent.futures import ThreadPoolExecutor
from functools import partial
from http.server import HTTPServer, SimpleHTTPRequestHandler
from socketserver import ThreadingMixIn

# Default pool size: covers agent's max concurrent connections (12 global)
# plus headroom for index fetches and config requests.
DEFAULT_POOL_SIZE = 16


class ThreadPoolHTTPServer(ThreadingMixIn, HTTPServer):
    """HTTPServer that dispatches requests to a bounded thread pool.

    Uses ThreadingMixIn for per-request thread dispatch, with the actual
    thread management delegated to a ThreadPoolExecutor. This bounds
    resource usage while handling concurrent connections from the agent.
    """

    daemon_threads = True
    allow_reuse_address = True

    def __init__(self, server_address, RequestHandlerClass, pool_size):
        super().__init__(server_address, RequestHandlerClass)
        self._pool = ThreadPoolExecutor(max_workers=pool_size)
        self._pool_size = pool_size

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

    def server_close(self):
        super().server_close()
        self._pool.shutdown(wait=False)


class RangeHTTPRequestHandler(SimpleHTTPRequestHandler):
    """SimpleHTTPRequestHandler with Range header support."""

    def send_head(self):
        """Serve a GET request, handling Range headers."""
        path = self.translate_path(self.path)
        if os.path.isdir(path):
            return super().send_head()

        try:
            f = open(path, "rb")  # noqa: SIM115
        except OSError:
            self.send_error(404, "File not found")
            return None

        fs = os.fstat(f.fileno())
        file_size = fs.st_size
        ctype = self.guess_type(path)

        range_header = self.headers.get("Range")
        if range_header is None:
            # No Range header — serve the full file as normal.
            self.send_response(200)
            self.send_header("Content-type", ctype)
            self.send_header("Content-Length", str(file_size))
            self.send_header("Accept-Ranges", "bytes")
            self.send_header("Last-Modified", self.date_time_string(fs.st_mtime))
            self.end_headers()
            return f

        # Parse "bytes=start-end" (single range only).
        try:
            range_spec = range_header.strip()
            if not range_spec.startswith("bytes="):
                raise ValueError
            range_spec = range_spec[6:]
            if "," in range_spec:
                # Multi-range not supported; fall back to full file.
                raise ValueError
            parts = range_spec.split("-", 1)
            start = int(parts[0]) if parts[0] else None
            end = int(parts[1]) if parts[1] else None
        except (ValueError, IndexError):
            f.close()
            self.send_error(416, "Requested Range Not Satisfiable")
            return None

        # Resolve start/end per RFC 7233.
        if start is not None and end is not None:
            if start > end or start >= file_size:
                f.close()
                self.send_error(416, "Requested Range Not Satisfiable")
                return None
            end = min(end, file_size - 1)
        elif start is not None:
            # "bytes=N-" means from N to end of file.
            if start >= file_size:
                f.close()
                self.send_error(416, "Requested Range Not Satisfiable")
                return None
            end = file_size - 1
        elif end is not None:
            # "bytes=-N" means last N bytes.
            start = max(0, file_size - end)
            end = file_size - 1
        else:
            f.close()
            self.send_error(416, "Requested Range Not Satisfiable")
            return None

        content_length = end - start + 1
        f.seek(start)

        self.send_response(206)
        self.send_header("Content-type", ctype)
        self.send_header("Content-Length", str(content_length))
        self.send_header("Content-Range", f"bytes {start}-{end}/{file_size}")
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("Last-Modified", self.date_time_string(fs.st_mtime))
        self.end_headers()

        # Return a wrapper that limits reads to content_length bytes.
        return _BoundedFile(f, content_length)

    def log_message(self, format, *args):
        """Thread-safe log output with thread name prefix."""
        thread = threading.current_thread().name
        sys.stderr.write(
            f"{thread} - {self.address_string()} - "
            f"[{self.log_date_time_string()}] {format % args}\n"
        )


class _BoundedFile:
    """File wrapper that limits reads to a byte count."""

    def __init__(self, f, limit):
        self._f = f
        self._remaining = limit

    def read(self, size=-1):
        if self._remaining <= 0:
            return b""
        if size < 0 or size > self._remaining:
            size = self._remaining
        data = self._f.read(size)
        self._remaining -= len(data)
        return data

    def close(self):
        self._f.close()


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
    args = parser.parse_args()

    root = os.path.abspath(args.directory)
    if not os.path.isdir(root):
        print(f"Error: {root} is not a directory", file=sys.stderr)
        sys.exit(1)

    # Pass directory= to SimpleHTTPRequestHandler via partial so
    # translate_path() resolves against the specified root, not CWD.
    handler = partial(RangeHTTPRequestHandler, directory=root)

    server = ThreadPoolHTTPServer(("", args.port), handler, args.threads)
    print(
        f"Serving {root} on port {args.port} with Range support "
        f"({args.threads} worker threads)"
    )
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nStopped.")
        server.server_close()


if __name__ == "__main__":
    main()
