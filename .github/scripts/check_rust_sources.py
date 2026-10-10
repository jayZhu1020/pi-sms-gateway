"""CI/development check: tracked Rust files must belong to Bazel Rust targets."""
import subprocess
import sys


def output(*args):
    return subprocess.check_output(args).decode()


tracked = set(output("git", "ls-files", "-z", "--", "*.rs").split("\0")) - {""}
labels = output("bazel", "query", 'labels(srcs, kind("rust_.* rule", //...))').splitlines()
registered = set()
for label in labels:
    if label.startswith("//"):
        package, name = label[2:].split(":", 1)
        registered.add(f"{package}/{name}" if package else name)
missing = sorted(tracked - registered)
if missing:
    print("Rust files missing from Bazel srcs:\n" + "\n".join(missing), file=sys.stderr)
    sys.exit(1)
print(f"All {len(tracked)} tracked Rust files are registered with Bazel.")
