"""Render the quote collage, tools/site/testimonials.html, to docs/testimonials.png (2800 x 2200).

Usage: python tools/site/testimonials.py [--port 4174] [--open]
Starts a local server and prints a URL. Open it in any Chromium-based browser (Edge, Chrome or Brave): the page draws itself at 2x onto a canvas and posts the PNG back, the server saves it and exits.
--open opens the URL in your default browser. Only the page and its fonts are served, on 127.0.0.1.
Python standard library only.
"""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote
import argparse
import posixpath
import struct
import threading
import webbrowser

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / 'docs' / 'testimonials.png'
PAGE = '/tools/site/testimonials.html'
FONTS = '/data/fonts/inter/'
PNG = b'\x89PNG\r\n\x1a\n'

class Handler(SimpleHTTPRequestHandler):
    def do_GET(self):
        path = posixpath.normpath(unquote(self.path.split('?', 1)[0].split('#', 1)[0]))
        if path != PAGE and not path.startswith(FONTS):
            self.send_error(404)
            return
        super().do_GET()

    def do_POST(self):
        length = int(self.headers.get('Content-Length') or 0)
        if self.path != '/save' or not 0 < length <= 30_000_000:
            self.send_error(400)
            return
        data = self.rfile.read(length)
        if not data.startswith(PNG):
            self.send_error(400, 'Not a PNG')
            return
        OUTPUT.write_bytes(data)
        width, height = struct.unpack('>II', data[16:24])
        message = f'Saved {OUTPUT.relative_to(ROOT).as_posix()} ({width}x{height}, {len(data) // 1024} KB)'
        self.send_response(200)
        self.send_header('Content-Type', 'text/plain; charset=utf-8')
        self.end_headers()
        self.wfile.write(message.encode())
        print(message, flush=True)
        threading.Thread(target=self.server.shutdown).start()

    def log_message(self, *args):
        pass

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--port', type=int, default=4174)
    parser.add_argument('--open', action='store_true', help='open the page in the default browser')
    args = parser.parse_args()
    server = ThreadingHTTPServer(('127.0.0.1', args.port), partial(Handler, directory=str(ROOT)))
    url = f'http://127.0.0.1:{args.port}{PAGE}?save'
    print(f'Open {url} to render and save the collage', flush=True)
    if args.open:
        webbrowser.open(url)
    server.serve_forever()
