from __future__ import annotations

import base64
import json
from itertools import count
from types import SimpleNamespace
from unittest.mock import MagicMock

import httpx
import pytest

from src.api.admin import provider_oauth as oauthmod


def _build_unsigned_jwt(payload: dict[str, object]) -> str:
    header = base64.urlsafe_b64encode(
        json.dumps({"alg": "none", "typ": "JWT"}, separators=(",", ":")).encode("utf-8")
    ).decode("ascii").rstrip("=")
    body = base64.urlsafe_b64encode(
        json.dumps(payload, separators=(",", ":")).encode("utf-8")
    ).decode("ascii").rstrip("=")
    return f"{header}.{body}."


@pytest.mark.asyncio
async def test_standard_batch_import_releases_db_connection_before_network_await(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    release_calls: list[str] = []

    monkeypatch.setattr(
        oauthmod,
        "_require_oauth_template",
        lambda _provider_type: SimpleNamespace(
            oauth=SimpleNamespace(
                token_url="https://example.com/oauth/token",
                client_id="client-id",
                client_secret=None,
                scopes=[],
            )
        ),
    )
    monkeypatch.setattr(
        oauthmod,
        "_parse_standard_oauth_import_entries",
        lambda _raw: [{"refresh_token": "r" * 120}],
    )
    monkeypatch.setattr(oauthmod, "_get_provider_api_formats", lambda _provider: [])
    monkeypatch.setattr(
        oauthmod,
        "_release_batch_import_db_connection_before_await",
        lambda _db: release_calls.append("release"),
    )

    async def _fake_post_oauth_token(**_kwargs: object) -> httpx.Response:
        raise RuntimeError("upstream unavailable")

    monkeypatch.setattr(oauthmod, "post_oauth_token", _fake_post_oauth_token)

    db = MagicMock()

    result = await oauthmod._batch_import_standard_oauth_internal(
        provider_id="provider-1",
        provider_type="codex",
        provider=SimpleNamespace(endpoints=[]),  # type: ignore[arg-type]
        raw_credentials="ignored",
        db=db,
        concurrency=1,
    )

    assert result.total == 1
    assert result.success == 0
    assert result.failed == 1
    assert release_calls
    db.commit.assert_not_called()


@pytest.mark.asyncio
async def test_standard_batch_import_commits_successes_in_chunks(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    key_ids = count(1)
    created_auth_configs: list[dict[str, object]] = []

    monkeypatch.setattr(
        oauthmod,
        "_PROVIDER_OAUTH_BATCH_IMPORT_COMMIT_BATCH_SIZE",
        2,
    )
    monkeypatch.setattr(
        oauthmod,
        "_require_oauth_template",
        lambda _provider_type: SimpleNamespace(
            oauth=SimpleNamespace(
                token_url="https://example.com/oauth/token",
                client_id="client-id",
                client_secret=None,
                scopes=[],
            )
        ),
    )
    monkeypatch.setattr(
        oauthmod,
        "_parse_standard_oauth_import_entries",
        lambda _raw: [{"refresh_token": f"r-{idx}" + ("x" * 120)} for idx in range(3)],
    )
    monkeypatch.setattr(
        oauthmod, "_get_provider_api_formats", lambda _provider: ["responses"]
    )
    monkeypatch.setattr(
        oauthmod,
        "_release_batch_import_db_connection_before_await",
        lambda _db: None,
    )

    async def _fake_post_oauth_token(**_kwargs: object) -> httpx.Response:
        idx = next(key_ids)
        return httpx.Response(
            200,
            json={
                "access_token": f"access-{idx}",
                "refresh_token": f"refresh-{idx}",
                "expires_in": 3600,
            },
            request=httpx.Request("POST", "https://example.com/oauth/token"),
        )

    async def _fake_enrich_auth_config(**kwargs: object) -> dict[str, object]:
        auth_config = dict(kwargs["auth_config"])  # type: ignore[call-overload]
        auth_config["email"] = f"user-{next(key_ids)}@example.com"
        auth_config["account_name"] = "Workspace Alpha"
        return auth_config

    created_ids = count(1)
    monkeypatch.setattr(oauthmod, "post_oauth_token", _fake_post_oauth_token)
    monkeypatch.setattr(oauthmod, "enrich_auth_config", _fake_enrich_auth_config)
    monkeypatch.setattr(
        oauthmod, "_check_duplicate_oauth_account", lambda *_args, **_kwargs: None
    )

    def _fake_create_oauth_key(*_args: object, **kwargs: object) -> SimpleNamespace:
        created_auth_configs.append(dict(kwargs["auth_config"]))
        return SimpleNamespace(id=f"key-{next(created_ids)}")

    monkeypatch.setattr(
        oauthmod,
        "_create_oauth_key",
        _fake_create_oauth_key,
    )

    db = MagicMock()

    result = await oauthmod._batch_import_standard_oauth_internal(
        provider_id="provider-1",
        provider_type="example",
        provider=SimpleNamespace(endpoints=[]),  # type: ignore[arg-type]
        raw_credentials="ignored",
        db=db,
        concurrency=1,
    )

    assert result.total == 3
    assert result.success == 3
    assert result.failed == 0
    assert db.commit.call_count == 2
    assert created_auth_configs[0]["account_name"] == "Workspace Alpha"


@pytest.mark.asyncio
async def test_standard_batch_import_accepts_access_token_without_refresh_exchange(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    access_token = _build_unsigned_jwt(
        {
            "exp": 1893456000,
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acc-1",
                "chatgpt_user_id": "user-1",
                "chatgpt_plan_type": "plus",
                "account_user_id": "user-1__acc-1",
            },
        }
    )
    created: dict[str, object] = {}

    monkeypatch.setattr(
        oauthmod,
        "_require_oauth_template",
        lambda _provider_type: SimpleNamespace(
            oauth=SimpleNamespace(
                token_url="https://example.com/oauth/token",
                client_id="client-id",
                client_secret=None,
                scopes=[],
            )
        ),
    )
    monkeypatch.setattr(
        oauthmod,
        "_parse_standard_oauth_import_entries",
        lambda _raw: [
            {
                "access_token": access_token,
                "refresh_token": "rt-1",
                "account_id": "acc-1",
                "account_user_id": "user-1__acc-1",
                "plan_type": "plus",
                "user_id": "user-1",
                "email": "user@example.com",
            }
        ],
    )
    monkeypatch.setattr(
        oauthmod,
        "_release_batch_import_db_connection_before_await",
        lambda _db: None,
    )
    monkeypatch.setattr(oauthmod, "_get_provider_api_formats", lambda _provider: ["responses"])
    async def _unexpected_post_oauth_token(**_kwargs: object) -> httpx.Response:
        raise AssertionError("should not refresh access-token import")

    monkeypatch.setattr(oauthmod, "post_oauth_token", _unexpected_post_oauth_token)

    async def _fake_enrich_auth_config(**kwargs: object) -> dict[str, object]:
        return dict(kwargs["auth_config"])  # type: ignore[call-overload]

    monkeypatch.setattr(oauthmod, "enrich_auth_config", _fake_enrich_auth_config)
    monkeypatch.setattr(
        oauthmod,
        "_check_duplicate_oauth_account",
        lambda *_args, **_kwargs: None,
    )

    def _fake_create_oauth_key(*_args: object, **kwargs: object) -> SimpleNamespace:
        created["access_token"] = kwargs["access_token"]
        created["auth_config"] = dict(kwargs["auth_config"])
        return SimpleNamespace(id="key-1")

    monkeypatch.setattr(oauthmod, "_create_oauth_key", _fake_create_oauth_key)

    db = MagicMock()

    result = await oauthmod._batch_import_standard_oauth_internal(
        provider_id="provider-1",
        provider_type="codex",
        provider=SimpleNamespace(endpoints=[]),  # type: ignore[arg-type]
        raw_credentials="ignored",
        db=db,
        concurrency=1,
    )

    assert result.total == 1
    assert result.success == 1
    assert result.failed == 0
    assert created["access_token"] == access_token
    assert created["auth_config"] == {
        "provider_type": "codex",
        "token_type": "Bearer",
        "refresh_token": "rt-1",
        "expires_at": 1893456000,
        "scope": None,
        "updated_at": created["auth_config"]["updated_at"],
        "account_id": "acc-1",
        "account_user_id": "user-1__acc-1",
        "plan_type": "plus",
        "user_id": "user-1",
        "email": "user@example.com",
    }
    assert isinstance(created["auth_config"]["updated_at"], int)


@pytest.mark.asyncio
async def test_import_refresh_token_supports_access_token_payload(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    access_token = _build_unsigned_jwt(
        {
            "exp": 1893456000,
            "email": "single@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acc-2",
                "chatgpt_user_id": "user-2",
                "chatgpt_plan_type": "team",
                "account_user_id": "user-2__acc-2",
            },
        }
    )

    provider = SimpleNamespace(id="provider-1", proxy=None, endpoints=[])
    created: dict[str, object] = {}
    scheduled: list[object] = []

    class _Query:
        def filter(self, *_args: object, **_kwargs: object) -> "_Query":
            return self

        def first(self) -> SimpleNamespace:
            return provider

    db = SimpleNamespace(query=lambda _model: _Query())

    monkeypatch.setattr(oauthmod, "_require_fixed_provider", lambda _provider: "codex")
    monkeypatch.setattr(oauthmod, "_resolve_proxy_for_oauth", lambda *_args, **_kwargs: (None, None))
    async def _unexpected_post_oauth_token(**_kwargs: object) -> httpx.Response:
        raise AssertionError("should not refresh access-token import")

    monkeypatch.setattr(oauthmod, "post_oauth_token", _unexpected_post_oauth_token)

    async def _fake_enrich_auth_config(**kwargs: object) -> dict[str, object]:
        return dict(kwargs["auth_config"])  # type: ignore[call-overload]

    monkeypatch.setattr(oauthmod, "enrich_auth_config", _fake_enrich_auth_config)
    monkeypatch.setattr(
        oauthmod,
        "_check_duplicate_oauth_account",
        lambda *_args, **_kwargs: None,
    )

    def _fake_create_oauth_key(*_args: object, **kwargs: object) -> SimpleNamespace:
        created["access_token"] = kwargs["access_token"]
        created["auth_config"] = dict(kwargs["auth_config"])
        return SimpleNamespace(id="key-9")

    monkeypatch.setattr(oauthmod, "_create_oauth_key", _fake_create_oauth_key)
    monkeypatch.setattr(oauthmod, "safe_create_task", lambda coro: scheduled.append(coro))

    payload = oauthmod.ImportRefreshTokenRequest(
        access_token=access_token,
        refresh_token="rt-single",
        account_id="acc-2",
        account_user_id="user-2__acc-2",
        user_id="user-2",
        plan_type="team",
        email="single@example.com",
    )

    result = await oauthmod.import_refresh_token(
        provider_id="provider-1",
        payload=payload,
        db=db,  # type: ignore[arg-type]
        _=SimpleNamespace(),
    )

    assert result.key_id == "key-9"
    assert result.provider_type == "codex"
    assert result.has_refresh_token is True
    assert result.email == "single@example.com"
    assert created["access_token"] == access_token
    assert created["auth_config"] == {
        "provider_type": "codex",
        "token_type": "Bearer",
        "refresh_token": "rt-single",
        "expires_at": 1893456000,
        "scope": None,
        "updated_at": created["auth_config"]["updated_at"],
        "account_id": "acc-2",
        "account_user_id": "user-2__acc-2",
        "plan_type": "team",
        "user_id": "user-2",
        "email": "single@example.com",
    }
    assert isinstance(created["auth_config"]["updated_at"], int)
    assert scheduled, "should trigger quota refresh after import"
    for coro in scheduled:
        close = getattr(coro, "close", None)
        if callable(close):
            close()


@pytest.mark.asyncio
async def test_kiro_batch_import_releases_db_connection_before_refresh(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    release_calls: list[str] = []

    class FakeKiroAuthConfig:
        def __init__(self, data: dict[str, object]) -> None:
            self._data = dict(data)
            self.provider_type = str(data.get("provider_type") or "")
            self.email = (
                data.get("email") if isinstance(data.get("email"), str) else None
            )
            self.auth_method = (
                data.get("auth_method")
                if isinstance(data.get("auth_method"), str)
                else "social"
            )
            self.refresh_token = str(data.get("refresh_token") or "")

        @staticmethod
        def validate_required_fields(
            _cred: dict[str, object],
        ) -> tuple[bool, str | None]:
            return True, None

        @classmethod
        def from_dict(cls, data: dict[str, object]) -> "FakeKiroAuthConfig":
            return cls(data)

        def to_dict(self) -> dict[str, object]:
            return dict(self._data)

    monkeypatch.setattr(
        oauthmod,
        "_parse_kiro_import_input",
        lambda _raw: [{"refresh_token": "r" * 120, "auth_method": "social"}],
    )
    monkeypatch.setattr(oauthmod, "_get_provider_api_formats", lambda _provider: [])
    monkeypatch.setattr(
        oauthmod,
        "_release_batch_import_db_connection_before_await",
        lambda _db: release_calls.append("release"),
    )
    monkeypatch.setattr(
        "src.services.provider.adapters.kiro.models.credentials.KiroAuthConfig",
        FakeKiroAuthConfig,
    )

    async def _fake_refresh_access_token(
        *_args: object, **_kwargs: object
    ) -> tuple[str, object]:
        raise RuntimeError("refresh token reused")

    monkeypatch.setattr(
        "src.services.provider.adapters.kiro.token_manager.refresh_access_token",
        _fake_refresh_access_token,
    )

    db = MagicMock()

    result = await oauthmod._batch_import_kiro_internal(
        provider_id="provider-1",
        provider=SimpleNamespace(endpoints=[]),  # type: ignore[arg-type]
        raw_credentials="ignored",
        db=db,
        concurrency=1,
    )

    assert result.total == 1
    assert result.success == 0
    assert result.failed == 1
    assert release_calls
    db.commit.assert_not_called()
