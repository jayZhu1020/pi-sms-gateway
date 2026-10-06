from datetime import datetime, timezone, timedelta
from uuid import uuid4
import pytest
from fastapi.testclient import TestClient
from cloud.app import create_app

DEVICE, ADMIN = 'd' * 40, 'a' * 40
PATH = '/v1/gateways/pi5/diagnostics'


def report():
    return dict(schema_version=1, report_id=str(uuid4()), observed_at=datetime.now(timezone.utc).isoformat(),
                modem_responding=True, sim='READY', signal=10, registration=5, sms_format=0,
                usb_audio='supported', query_errors=[])


def headers(token):
    return {'Authorization': 'Bearer ' + token}


@pytest.fixture
def client(tmp_path):
    return TestClient(create_app(str(tmp_path / 'db.sqlite3'), DEVICE, ADMIN))


def test_auth_and_unknown_gateway(client):
    assert client.put(PATH, json=report()).status_code == 401
    assert client.put(PATH, json=report(), headers=headers(ADMIN)).status_code == 401
    assert client.get(PATH, headers=headers(DEVICE)).status_code == 401
    assert client.get(PATH.replace('pi5', 'another'), headers=headers(ADMIN)).status_code == 404


def test_persistence_replay_and_conflict(tmp_path):
    db = str(tmp_path / 'db.sqlite3')
    first = TestClient(create_app(db, DEVICE, ADMIN))
    r = report()
    assert first.put(PATH, json=r, headers=headers(DEVICE)).status_code == 200
    assert first.put(PATH, json=r, headers=headers(DEVICE)).json()['duplicate']
    second = TestClient(create_app(db, DEVICE, ADMIN))
    assert second.get(PATH, headers=headers(ADMIN)).json()['report']['sim'] == 'READY'
    r['signal'] = 9
    assert second.put(PATH, json=r, headers=headers(DEVICE)).status_code == 409


def test_stale_future_and_extra_fields(client):
    r = report()
    assert client.put(PATH, json=r, headers=headers(DEVICE)).status_code == 200
    stale = report()
    stale['observed_at'] = (datetime.now(timezone.utc) - timedelta(minutes=1)).isoformat()
    assert client.put(PATH, json=stale, headers=headers(DEVICE)).status_code == 409
    stale['observed_at'] = (datetime.now(timezone.utc) + timedelta(hours=1)).isoformat()
    assert client.put(PATH, json=stale, headers=headers(DEVICE)).status_code == 422
    r['sms_body'] = 'must not be accepted'
    assert client.put(PATH, json=r, headers=headers(DEVICE)).status_code == 422


def test_large_body(client):
    assert client.put(PATH, content=b'x' * 9000, headers=headers(DEVICE)).status_code == 413


def test_tokens_required(tmp_path):
    with pytest.raises(ValueError):
        create_app(str(tmp_path / 'db'), 'short', ADMIN)
