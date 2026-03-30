from __future__ import annotations

from types import SimpleNamespace
from unittest.mock import MagicMock

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient

from src.api.admin.access_token_deletions import router as deletion_router
from src.database import get_db
from src.utils.auth_utils import require_admin


@pytest.fixture
def admin_access_token_deletions_app(monkeypatch: pytest.MonkeyPatch) -> tuple[TestClient, MagicMock]:
    db = MagicMock()
    app = FastAPI()
    app.include_router(deletion_router)
    app.dependency_overrides[get_db] = lambda: db
    app.dependency_overrides[require_admin] = lambda: SimpleNamespace(id='admin-1')
    return TestClient(app), db


def test_access_token_deletion_summary_route_returns_counts(
    admin_access_token_deletions_app: tuple[TestClient, MagicMock],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, db = admin_access_token_deletions_app
    monkeypatch.setattr(
        'src.api.admin.access_token_deletions.get_access_token_delete_summary',
        lambda db, days=1: {'total': 7, 'today': 3, 'last_24h': 4},
    )

    response = client.get('/api/admin/access-token-deletions/summary')

    assert response.status_code == 200
    assert response.json()['total'] == 7
    assert response.json()['today'] == 3
    assert response.json()['last_24h'] == 4
    assert db is not None


def test_access_token_deletion_list_route_passes_filters(
    admin_access_token_deletions_app: tuple[TestClient, MagicMock],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, _db = admin_access_token_deletions_app
    captured: dict[str, object] = {}

    def _fake_list(db, **kwargs):
        captured.update(kwargs)
        return {'total': 1, 'items': [{'deleted_key_id': 'k1'}]}

    monkeypatch.setattr(
        'src.api.admin.access_token_deletions.list_access_token_delete_logs',
        _fake_list,
    )

    response = client.get('/api/admin/access-token-deletions?email=demo@test.local&days=7&limit=20&offset=0')

    assert response.status_code == 200
    assert response.json()['total'] == 1
    assert captured == {
        'email': 'demo@test.local',
        'provider_id': None,
        'days': 7,
        'limit': 20,
        'offset': 0,
    }
