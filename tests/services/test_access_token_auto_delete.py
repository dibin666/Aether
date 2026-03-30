from __future__ import annotations

from types import SimpleNamespace

from src.models.database import AccessTokenDeleteLog
from src.services.provider_keys.access_token_auto_delete import (
    build_delete_log_payload,
    is_access_token_only_oauth_key,
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
