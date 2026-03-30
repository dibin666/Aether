from __future__ import annotations

from types import SimpleNamespace
from typing import Any

import pytest

from src.models.database import AccessTokenDeleteLog
from src.services.provider_keys.access_token_auto_delete import (
    build_delete_log_payload,
    delete_access_token_only_key_on_http400,
    is_access_token_only_oauth_key,
    reset_access_token_only_key_http400_counter,
)


def test_is_access_token_only_oauth_key_only_matches_codex_oauth_without_refresh_token() -> None:
    provider = SimpleNamespace(id='p1', provider_type='codex')
    key = SimpleNamespace(
        id='k1',
        provider_id='p1',
        auth_type='oauth',
        is_active=True,
        api_key='enc-access',
        auth_config='enc-config',
    )

    decrypt_map = {
        'enc-access': 'access-token-value',
        'enc-config': '{"email": "demo@test.local"}',
    }

    def _decrypt(value: str) -> str:
        return decrypt_map[value]

    assert is_access_token_only_oauth_key(provider=provider, key=key, decrypt=_decrypt) is True


def test_build_delete_log_payload_keeps_display_fields() -> None:
    provider = SimpleNamespace(id='p1', name='Codex Pool', provider_type='codex')
    key = SimpleNamespace(id='k1', provider_id='p1', name='demo-key', auth_type='oauth')

    payload = build_delete_log_payload(
        provider=provider,
        key=key,
        oauth_email='demo@test.local',
        trigger_status_code=400,
        endpoint_sig='openai:cli',
        proxy_node_id='node-1',
        proxy_node_name='CF-1',
        request_id='req-123',
        error_message='400 Bad Request',
        raw_error_excerpt='<html>400 Bad Request</html>',
    )

    assert payload['deleted_key_id'] == 'k1'
    assert payload['provider_id'] == 'p1'
    assert payload['provider_name'] == 'Codex Pool'
    assert payload['oauth_email'] == 'demo@test.local'
    assert payload['trigger_status_code'] == 400
    assert payload['endpoint_sig'] == 'openai:cli'
    assert payload['proxy_node_name'] == 'CF-1'
    assert payload['deleted_by'] == 'system:auto-delete-http400'


def test_access_token_delete_log_model_has_expected_columns() -> None:
    columns = {column.name for column in AccessTokenDeleteLog.__table__.columns}

    assert {'deleted_key_id', 'provider_id', 'oauth_email', 'trigger_status_code', 'deleted_at'} <= columns


class _FakeDB:
    def __init__(self, key: Any | None = None) -> None:
        self.key = key
        self.deleted: list[Any] = []
        self.added: list[Any] = []
        self.commit_count = 0
        self.rollback_count = 0

    def add(self, obj: Any) -> None:
        self.added.append(obj)

    def delete(self, obj: Any) -> None:
        self.deleted.append(obj)

    def commit(self) -> None:
        self.commit_count += 1

    def rollback(self) -> None:
        self.rollback_count += 1


@pytest.mark.asyncio
async def test_delete_access_token_only_key_on_http400_deletes_key_and_records_log(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    key = SimpleNamespace(
        id='k1',
        provider_id='p1',
        name='demo-key',
        auth_type='oauth',
        is_active=True,
        api_key='enc-access',
        auth_config='enc-config',
        allowed_models=['gpt-5.2'],
        upstream_metadata={},
    )
    provider = SimpleNamespace(id='p1', name='Codex Pool', provider_type='codex')
    db = _FakeDB(key)

    monkeypatch.setattr(
        'src.services.provider_keys.access_token_auto_delete.crypto_service.decrypt',
        lambda value: {
            'enc-access': 'access-token',
            'enc-config': '{"email": "demo@test.local"}',
        }[value],
    )
    monkeypatch.setattr(
        'src.services.provider_keys.access_token_auto_delete._get_key_for_delete',
        lambda db, key_id: key,
    )
    monkeypatch.setattr(
        'src.services.provider_keys.access_token_auto_delete.cleanup_key_references',
        lambda db, key_ids: None,
    )
    monkeypatch.setattr(
        'src.services.provider_keys.access_token_auto_delete.run_delete_key_side_effects',
        lambda **kwargs: None,
    )

    deleted_1 = await delete_access_token_only_key_on_http400(
        db=db,
        provider=provider,
        key_id='k1',
        status_code=400,
        endpoint_sig='openai:cli',
        request_id='req-1',
        error_message='400 Bad Request',
        raw_error_excerpt='<html>400</html>',
        proxy_node_id='node-1',
        proxy_node_name='CF-1',
    )
    deleted_2 = await delete_access_token_only_key_on_http400(
        db=db,
        provider=provider,
        key_id='k1',
        status_code=400,
        endpoint_sig='openai:cli',
        request_id='req-2',
        error_message='400 Bad Request',
        raw_error_excerpt='<html>400</html>',
        proxy_node_id='node-1',
        proxy_node_name='CF-1',
    )
    deleted_3 = await delete_access_token_only_key_on_http400(
        db=db,
        provider=provider,
        key_id='k1',
        status_code=400,
        endpoint_sig='openai:cli',
        request_id='req-3',
        error_message='400 Bad Request',
        raw_error_excerpt='<html>400</html>',
        proxy_node_id='node-1',
        proxy_node_name='CF-1',
    )

    assert deleted_1 is False
    assert deleted_2 is False
    assert deleted_3 is True
    assert db.deleted == [key]
    assert db.commit_count == 1
    assert len(db.added) == 1
    assert db.added[0].deleted_key_id == 'k1'


@pytest.mark.asyncio
async def test_delete_access_token_only_key_on_http400_skips_refresh_token_and_non_400(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    key = SimpleNamespace(
        id='k1',
        provider_id='p1',
        name='demo-key',
        auth_type='oauth',
        is_active=True,
        api_key='enc-access',
        auth_config='enc-config',
        allowed_models=[],
        upstream_metadata={},
    )
    provider = SimpleNamespace(id='p1', name='Codex Pool', provider_type='codex')

    monkeypatch.setattr(
        'src.services.provider_keys.access_token_auto_delete.crypto_service.decrypt',
        lambda value: {
            'enc-access': 'access-token',
            'enc-config': '{"refresh_token": "rt-1", "email": "demo@test.local"}',
        }[value],
    )
    monkeypatch.setattr(
        'src.services.provider_keys.access_token_auto_delete._get_key_for_delete',
        lambda db, key_id: key,
    )

    deleted = await delete_access_token_only_key_on_http400(
        db=SimpleNamespace(),
        provider=provider,
        key_id='k1',
        status_code=400,
        endpoint_sig='openai:cli',
        request_id='req-1',
        error_message='400 Bad Request',
        raw_error_excerpt='400',
        proxy_node_id=None,
        proxy_node_name=None,
    )

    assert deleted is False


def test_reset_access_token_only_key_http400_counter_clears_existing_counter(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    key = SimpleNamespace(
        id='k1',
        upstream_metadata={
            'codex': {
                'access_token_delete_http400_count': 2,
                'access_token_delete_last_http400_at': '2026-03-30T00:00:00+00:00',
                'legacy_marker': 'keep-me',
            }
        },
    )
    monkeypatch.setattr(
        'src.services.provider_keys.access_token_auto_delete._get_key_for_delete',
        lambda db, key_id: key,
    )

    reset = reset_access_token_only_key_http400_counter(db=SimpleNamespace(), key_id='k1')

    assert reset is True
    assert key.upstream_metadata['codex']['legacy_marker'] == 'keep-me'
    assert 'access_token_delete_http400_count' not in key.upstream_metadata['codex']
    assert 'access_token_delete_last_http400_at' not in key.upstream_metadata['codex']


@pytest.mark.asyncio
async def test_delete_access_token_only_key_on_http400_is_idempotent_when_key_missing(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    provider = SimpleNamespace(id='p1', name='Codex Pool', provider_type='codex')
    monkeypatch.setattr(
        'src.services.provider_keys.access_token_auto_delete._get_key_for_delete',
        lambda db, key_id: None,
    )

    deleted = await delete_access_token_only_key_on_http400(
        db=SimpleNamespace(),
        provider=provider,
        key_id='missing-key',
        status_code=400,
        endpoint_sig='openai:cli',
        request_id='req-2',
        error_message='400 Bad Request',
        raw_error_excerpt='400',
        proxy_node_id=None,
        proxy_node_name=None,
    )

    assert deleted is False
