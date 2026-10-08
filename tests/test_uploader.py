import json
from http.server import BaseHTTPRequestHandler, HTTPServer
import threading
import pytest
from device.publish_report import publish


def test_rejects_unsafe_destinations():
    for url, gateway in [('http://example.com', 'pi5'),
                         ('https://example.com', '../other'),
                         ('https://user:pass@example.com', 'pi5')]:
        with pytest.raises(ValueError):
            publish(url, gateway, 'secret', {})


def test_upload_and_refuse_redirect():
    class Handler(BaseHTTPRequestHandler):
        redirect = False
        def do_PUT(self):
            assert self.path == '/v1/gateways/pi5/diagnostics'
            assert self.headers['Authorization'] == 'Bearer test-token'
            assert json.loads(self.rfile.read(int(self.headers['Content-Length']))) == {'schema_version': 1}
            self.send_response(307 if self.redirect else 200)
            if self.redirect:
                self.send_header('Location', '/other')
            self.end_headers()
            self.wfile.write(b'{"accepted": true}')
        def log_message(self, *args):
            pass
    server = HTTPServer(('127.0.0.1', 0), Handler)
    thread = threading.Thread(target=server.serve_forever)
    thread.start()
    try:
        url = f'http://127.0.0.1:{server.server_port}'
        assert publish(url, 'pi5', 'test-token', {'schema_version': 1})['accepted']
        Handler.redirect = True
        from urllib.error import HTTPError
        with pytest.raises(HTTPError) as error:
            publish(url, 'pi5', 'test-token', {'schema_version': 1})
        assert error.value.code == 307
    finally:
        server.shutdown()
        thread.join()
        server.server_close()
