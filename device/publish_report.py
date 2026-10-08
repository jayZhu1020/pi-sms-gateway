"""Upload a saved diagnostic report. No cellular operations are exposed."""
import argparse
import json
from pathlib import Path
from urllib.parse import urlsplit
from urllib.request import Request, build_opener, HTTPRedirectHandler
from urllib.error import URLError


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None  # Never forward credentials to a redirected destination.


def publish(url, gateway, token, report):
    parts = urlsplit(url)
    if parts.username or parts.password or parts.query or parts.fragment:
        raise ValueError("Use a plain proxy base URL without credentials or query")
    if parts.scheme != "https" and not (
        parts.scheme == "http" and parts.hostname in {"127.0.0.1", "localhost", "::1"}
    ):
        raise ValueError("HTTPS required except for localhost SSH tunnels")
    if not gateway or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in gateway):
        raise ValueError("Invalid gateway ID")
    request = Request(url.rstrip('/') + f'/v1/gateways/{gateway}/diagnostics',
                      data=json.dumps(report).encode(), method='PUT',
                      headers={'Authorization': 'Bearer ' + token,
                               'Content-Type': 'application/json'})
    with build_opener(NoRedirect).open(request, timeout=15) as response:
        return json.load(response)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url', required=True)
    parser.add_argument('--gateway', default='pi5')
    parser.add_argument('--token-file', required=True)
    parser.add_argument('--report', required=True)
    args = parser.parse_args()
    try:
        result = publish(args.url, args.gateway, Path(args.token_file).read_text().strip(),
                         json.loads(Path(args.report).read_text()))
    except (OSError, ValueError, URLError):
        parser.exit(1, 'Upload failed. Check URL, token, report, and connectivity.\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
