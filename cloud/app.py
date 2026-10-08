"""Single-gateway diagnostic proxy foundation, not an SMS or voice relay yet."""
from datetime import datetime, timezone, timedelta
import hmac
import json
import os
from pathlib import Path
import sqlite3
from typing import Literal
from uuid import UUID

from fastapi import Depends, FastAPI, HTTPException
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer
from pydantic import AwareDatetime, BaseModel, ConfigDict, Field


class DiagnosticReport(BaseModel):
    model_config = ConfigDict(extra='forbid')
    schema_version: Literal[1]
    report_id: UUID
    observed_at: AwareDatetime
    modem_responding: bool
    sim: Literal['READY', 'SIM PIN', 'SIM PUK'] | None
    signal: int | None = Field(ge=0, le=99)
    registration: int | None = Field(ge=0, le=10)
    sms_format: Literal[0, 1] | None
    usb_audio: Literal['supported'] | None
    query_errors: list[Literal['sim', 'signal', 'registration', 'sms_format', 'usb_audio']] = Field(max_length=5)


def create_app(database=None, device_token=None, admin_token=None, gateway_id=None):
    database = database or os.environ.get('PROXY_DATABASE', 'data/proxy.sqlite3')
    device_token = device_token or os.environ.get('PROXY_DEVICE_TOKEN', '')
    admin_token = admin_token or os.environ.get('PROXY_ADMIN_TOKEN', '')
    gateway_id = gateway_id or os.environ.get('PROXY_GATEWAY_ID', 'pi5')
    if min(len(device_token), len(admin_token)) < 32 or device_token == admin_token:
        raise ValueError('Set distinct device/admin tokens of at least 32 characters')
    Path(database).parent.mkdir(parents=True, exist_ok=True)
    # The database retains only the latest sanitized report per configured gateway.
    with sqlite3.connect(database) as db:
        db.execute('CREATE TABLE IF NOT EXISTS diagnostics (gateway TEXT PRIMARY KEY, report_id TEXT, observed_at TEXT, received_at TEXT, payload TEXT)')
    os.chmod(database, 0o600)
    app = FastAPI(title='Pi gateway diagnostic proxy', version='0.1.0', docs_url=None, redoc_url=None, openapi_url=None)
    bearer = HTTPBearer(auto_error=False)

    def authorize(expected):
        def check(credentials: HTTPAuthorizationCredentials | None = Depends(bearer)):
            if credentials is None or not hmac.compare_digest(credentials.credentials.encode(), expected.encode()):
                raise HTTPException(401, 'Unauthorized', headers={'WWW-Authenticate': 'Bearer'})
        return check

    class BodyLimit:
        def __init__(self, app):
            self.app = app

        async def __call__(self, scope, receive, send):
            if scope['type'] != 'http':
                return await self.app(scope, receive, send)
            messages, size = [], 0
            while True:
                message = await receive()
                if message['type'] == 'http.disconnect':
                    return
                size += len(message.get('body', b''))
                if size > 8192:
                    await send({'type': 'http.response.start', 'status': 413, 'headers': []})
                    await send({'type': 'http.response.body', 'body': b'Report too large'})
                    return
                messages.append(message)
                if not message.get('more_body', False):
                    break
            async def replay():
                return messages.pop(0) if messages else await receive()
            await self.app(scope, replay, send)

    app.add_middleware(BodyLimit)

    @app.get('/healthz')
    def health():
        with sqlite3.connect(database) as db:
            db.execute('SELECT 1 FROM diagnostics LIMIT 1')
        return {'status': 'ok', 'scope': 'diagnostics-only'}

    def check_gateway(gateway):
        if gateway != gateway_id:
            raise HTTPException(404, 'Gateway not found')

    @app.put('/v1/gateways/{gateway}/diagnostics', dependencies=[Depends(authorize(device_token))])
    def ingest(gateway: str, report: DiagnosticReport):
        check_gateway(gateway)
        now = datetime.now(timezone.utc)
        observed = report.observed_at.astimezone(timezone.utc)
        if observed > now + timedelta(minutes=5) or observed < now - timedelta(hours=24):
            raise HTTPException(422, 'Report must be within the last 24 hours; check device clock')
        payload = report.model_dump_json()
        with sqlite3.connect(database, timeout=5) as db:
            db.execute('BEGIN IMMEDIATE')
            row = db.execute('SELECT report_id, observed_at, payload FROM diagnostics WHERE gateway=?', (gateway,)).fetchone()
            if row:
                if row[0] == str(report.report_id):
                    if row[2] != payload:
                        raise HTTPException(409, 'Report ID already has different content')
                    return {'accepted': True, 'duplicate': True}
                if observed <= datetime.fromisoformat(row[1]):
                    raise HTTPException(409, 'A newer or equal-time report already exists')
            db.execute('INSERT OR REPLACE INTO diagnostics VALUES (?, ?, ?, ?, ?)',
                       (gateway, str(report.report_id), observed.isoformat(), now.isoformat(), payload))
        return {'accepted': True, 'duplicate': False}

    @app.get('/v1/gateways/{gateway}/diagnostics', dependencies=[Depends(authorize(admin_token))])
    def latest(gateway: str):
        check_gateway(gateway)
        with sqlite3.connect(database) as db:
            row = db.execute('SELECT received_at, payload FROM diagnostics WHERE gateway=?', (gateway,)).fetchone()
        if row is None:
            raise HTTPException(404, 'No report received')
        received = datetime.fromisoformat(row[0])
        return {'received_at': row[0], 'age_seconds': max(0, (datetime.now(timezone.utc) - received).total_seconds()),
                'report': json.loads(row[1])}

    return app
