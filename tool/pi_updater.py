"""Build locally; install/test/rollback/uninstall a pure-Python probe over SSH."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys
import zipfile

# Executed on the Pi. Only wheel installation occurs here, never package building.
REMOTE = r'''
import fcntl, hashlib, json, os, pathlib, re, shutil, subprocess, sys, venv
options = json.loads(sys.argv[1])
root = pathlib.Path(options['root']).expanduser().absolute()
action = options['action']
if not re.fullmatch(r'pi-sim-device(?:-test-[a-z0-9]+)?', root.name) or root.is_symlink():
    raise ValueError('Use a managed pi-sim-device directory, not a symlink')
root.parent.mkdir(parents=True, exist_ok=True)
with open(root.parent / ('.' + root.name + '.lock'), 'a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    marker = root / '.managed'
    if root.exists() and (not marker.is_file() or marker.read_text() != 'pi-sim-device'):
        raise ValueError('Refusing an unmarked directory')
    if action == 'install' and not root.exists():
        root.mkdir(mode=0o700)
        marker.write_text('pi-sim-device')
    if not root.exists():
        if action in {'status', 'uninstall'}:
            print(json.dumps({'installed': False})); sys.exit(0)
        raise ValueError('No managed installation exists')
    releases = root / 'releases'
    current = root / 'current'
    def run(*args):
        subprocess.run(args, check=True, timeout=90, stdout=sys.stderr)
    def smoke(release):
        run(str(release / 'venv/bin/sim-probe'), '--help')
    def activate(release):
        temporary = root / 'current.new'
        temporary.unlink(missing_ok=True)
        temporary.symlink_to(release)
        os.replace(temporary, current)
    if action == 'install':
        data = sys.stdin.buffer.read(10 * 1024 * 1024 + 1)
        digest = hashlib.sha256(data).hexdigest()
        if len(data) > 10 * 1024 * 1024 or digest != options['digest']:
            raise ValueError('Artifact integrity check failed')
        release = releases / digest
        releases.mkdir(exist_ok=True)
        if not release.exists():
            release.mkdir()
            try:
                wheel = release / options['filename']
                wheel.write_bytes(data)
                venv.create(release / 'venv', with_pip=True)
                run(str(release / 'venv/bin/python'), '-m', 'pip', 'install',
                    '--no-index', '--no-deps', '--no-compile', '--disable-pip-version-check', str(wheel))
                smoke(release)
                (release / '.ready').touch()
            except BaseException:
                shutil.rmtree(release)
                raise
        if not (release / '.ready').is_file():
            raise ValueError('Release is incomplete')
        smoke(release)
        activate(release)
        print(json.dumps({'installed': True, 'release': digest}))
    elif action == 'rollback':
        digest = options['release']
        if not re.fullmatch('[a-f0-9]{64}', digest):
            raise ValueError('Invalid release ID')
        release = releases / digest
        if not (release / '.ready').is_file():
            raise ValueError('Release not available')
        smoke(release)
        activate(release)
        print(json.dumps({'release': digest}))
    elif action == 'uninstall':
        shutil.rmtree(root)
        print(json.dumps({'installed': False}))
    elif action == 'status':
        print(json.dumps({'installed': current.is_symlink(),
                          'release': current.resolve().name if current.is_symlink() else None,
                          'available': sorted(p.parent.name for p in releases.glob('*/.ready'))}))
    elif action == 'test':
        if not current.is_symlink():
            raise ValueError('No active release')
        run(str(current / 'venv/bin/sim-probe'))
        print(json.dumps({'probe_completed': True}))
'''


def artifact(path):
    path = Path(path)
    if not re.fullmatch(r'pi_sim_device-[\w.]+-py3-none-any\.whl', path.name):
        raise ValueError('Expected an architecture-independent pi-sim-device wheel')
    data = path.read_bytes()
    if len(data) > 10 * 1024 * 1024:
        raise ValueError('Artifact exceeds 10 MiB')
    with zipfile.ZipFile(path) as wheel:
        metadata = [n for n in wheel.namelist() if n.endswith('.dist-info/METADATA')]
        if len(metadata) != 1 or '\nName: pi-sim-device\n' not in wheel.read(metadata[0]).decode():
            raise ValueError('Package identity mismatch')
    return data, hashlib.sha256(data).hexdigest()


def execute(host, options, data=b'', alias=None):
    if not re.fullmatch(r'[A-Za-z0-9_.@:-]+', host) or host.startswith('-'):
        raise ValueError('Invalid SSH destination')
    command = ['ssh', '-o', 'BatchMode=yes', '-o', 'StrictHostKeyChecking=yes',
               '-o', 'ConnectTimeout=10']
    if alias:
        command += ['-o', 'HostKeyAlias=' + alias]
    command += [host, shlex.join(['python3', '-c', REMOTE, json.dumps(options)])]
    result = subprocess.run(command, input=data, capture_output=True, timeout=180)
    if result.returncode:
        sys.stderr.buffer.write(result.stderr)
        raise RuntimeError('Remote operation failed; active release was not selected on failed install')
    if options['action'] == 'test':
        sys.stderr.buffer.write(result.stderr)
    return json.loads(result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['build', 'install', 'update', 'status', 'test', 'rollback', 'uninstall'])
    parser.add_argument('--source', default='.', help='Repository with pyproject.toml (build only)')
    parser.add_argument('--out', default='/tmp/pi-sim-artifacts', help='Local build artifact directory')
    parser.add_argument('--host', help='SSH user@host; required for remote actions')
    parser.add_argument('--host-key-alias', help='Existing known-host name for the same Pi')
    parser.add_argument('--root', default='~/.local/share/pi-sim-device', help='Managed remote installation directory')
    parser.add_argument('--wheel', help='Local pure-Python wheel to install')
    parser.add_argument('--release', help='SHA-256 release ID for rollback')
    args = parser.parse_args()
    try:
        if args.action == 'build':
            subprocess.run([sys.executable, '-m', 'build', '--wheel', '--outdir',
                            str(Path(args.out).resolve()), str(Path(args.source).resolve())], check=True)
            return
        if not args.host:
            parser.error('--host is required')
        options = dict(action='install' if args.action == 'update' else args.action, root=args.root)
        data = b''
        if args.action in {'install', 'update'}:
            if not args.wheel:
                parser.error('--wheel is required')
            data, digest = artifact(args.wheel)
            options.update(digest=digest, filename=Path(args.wheel).name)
        if args.action == 'rollback':
            options['release'] = args.release or ''
        print(json.dumps(execute(args.host, options, data, args.host_key_alias)))
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, zipfile.BadZipFile) as error:
        parser.exit(1, str(error) + '\n')


if __name__ == '__main__':
    main()
