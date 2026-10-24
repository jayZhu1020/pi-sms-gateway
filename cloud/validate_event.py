"""Inspect a private event file without printing sender or message contents."""

import argparse
from pathlib import Path
import sys

from cloud.events import InvalidEvent, MAX_REQUEST_BYTES, parse_event


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("file", type=Path)
    parser.add_argument("--gateway-id", required=True)
    args = parser.parse_args()
    try:
        with args.file.open("rb") as source:
            raw = source.read(MAX_REQUEST_BYTES + 1)
        parse_event(raw, authenticated_gateway_id=args.gateway_id)
    except OSError:
        print("Cannot read event file", file=sys.stderr)
        return 1
    except InvalidEvent as error:
        print(f"Invalid event: {error}", file=sys.stderr)
        return 1
    print("Valid incoming SMS event (contents withheld)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
