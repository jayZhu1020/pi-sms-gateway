import json
from pathlib import Path
import subprocess
import sys
import zipfile
import pytest
from tool.pi_updater import REMOTE, artifact, execute


def wheel(tmp_path, bad=False):
    path = tmp_path / 'pi_sim_device-0.1.0-py3-none-any.whl'
    info = 'pi_sim_device-0.1.0.dist-info/'
    with zipfile.ZipFile(path, 'w') as z:
        z.writestr('fixture.py', 'def main():\n    '+ ('raise RuntimeError("failed")' if bad else 'print("probe help")')+'\n')
        z.writestr(info+'METADATA', 'Metadata-Version: 2.1\nName: pi-sim-device\nVersion: 0.1.0\n')
        z.writestr(info+'WHEEL', 'Wheel-Version: 1.0\nRoot-Is-Purelib: true\nTag: py3-none-any\n')
        z.writestr(info+'entry_points.txt', '[console_scripts]\nsim-probe = fixture:main\n')
        z.writestr(info+'RECORD', '')
    return path


def run(root, action, data=b'', **options):
    return subprocess.run([sys.executable, '-c', REMOTE,
                           json.dumps(dict(root=str(root), action=action, **options))],
                          input=data, capture_output=True, timeout=120)


def test_lifecycle_and_failed_candidate(tmp_path):
    root = tmp_path / 'pi-sim-device'
    package = wheel(tmp_path)
    data, digest = artifact(package)
    assert run(root, 'install', data, digest=digest, filename=package.name).returncode == 0
    assert run(root, 'install', data, digest=digest, filename=package.name).returncode == 0
    assert json.loads(run(root, 'status').stdout)['release'] == digest
    bad_data, bad_digest = artifact(wheel(tmp_path, bad=True))
    assert run(root, 'install', bad_data, digest=bad_digest, filename=package.name).returncode != 0
    assert json.loads(run(root, 'status').stdout)['release'] == digest
    assert run(root, 'rollback', release=digest).returncode == 0
    sentinel = tmp_path / 'other-file'
    sentinel.write_text('keep')
    assert run(root, 'uninstall').returncode == 0
    assert not root.exists() and sentinel.read_text() == 'keep'


def test_rejects_unmanaged_root_bad_hash_and_host(tmp_path):
    root = tmp_path / 'pi-sim-device'
    root.mkdir()
    assert run(root, 'uninstall').returncode != 0
    package = wheel(tmp_path)
    data, _ = artifact(package)
    other = tmp_path / 'pi-sim-device-test-hash'
    assert run(other, 'install', data, digest='0'*64, filename=package.name).returncode != 0
    assert not (other / 'current').exists()
    with pytest.raises(ValueError):
        execute('-oProxyCommand=bad', {'action': 'status'})
