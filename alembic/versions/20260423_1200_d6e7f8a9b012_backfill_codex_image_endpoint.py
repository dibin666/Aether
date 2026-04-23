"""backfill_codex_image_endpoint

Backfill Codex reverse-proxy endpoints:
- ensure `openai:image` endpoint exists and is pinned to force_stream
- ensure provider keys include `openai:image`

Revision ID: d6e7f8a9b012
Revises: b5c6d7e8f901
Create Date: 2026-04-23 12:00:00.000000+00:00
"""

from __future__ import annotations

import json
import uuid
from typing import Any

import sqlalchemy as sa

from alembic import op

revision = "d6e7f8a9b012"
down_revision = "b5c6d7e8f901"
branch_labels = None
depends_on = None

_CODEX_BASE_URL = "https://chatgpt.com/backend-api/codex"
_CLI_FORMAT = "openai:cli"
_IMAGE_FORMAT = "openai:image"
_FORCE_STREAM = "force_stream"


def _find_codex_provider_ids(conn: sa.Connection) -> list[str]:
    rows = conn.execute(
        sa.text(
            """
        SELECT DISTINCT p.id
        FROM providers p
        LEFT JOIN provider_endpoints pe ON pe.provider_id = p.id
        WHERE lower(COALESCE(p.provider_type, '')) = 'codex'
           OR (
                lower(COALESCE(pe.api_format, '')) IN ('openai:cli', 'openai:image')
            AND lower(COALESCE(pe.base_url, '')) LIKE '%/backend-api/codex%'
           )
        """
        )
    )
    return [str(r[0]) for r in rows if r[0]]


def _get_cli_endpoint(conn: sa.Connection, provider_id: str) -> dict[str, Any] | None:
    row = (
        conn.execute(
            sa.text(
                """
                SELECT base_url, header_rules, body_rules, max_retries, proxy, config
                FROM provider_endpoints
                WHERE provider_id = :pid AND api_format = :fmt
                LIMIT 1
            """
            ),
            {"pid": provider_id, "fmt": _CLI_FORMAT},
        )
        .mappings()
        .first()
    )
    return dict(row) if row else None


def _json(value: Any) -> str | None:
    return json.dumps(value, ensure_ascii=False) if value is not None else None


def _build_force_stream_config(raw_config: Any) -> dict[str, Any]:
    cfg = dict(raw_config or {}) if isinstance(raw_config, dict) else {}
    cfg.pop("upstreamStreamPolicy", None)
    cfg.pop("upstream_stream", None)
    cfg["upstream_stream_policy"] = _FORCE_STREAM
    return cfg


def _ensure_image_endpoint(conn: sa.Connection, provider_id: str, cli: dict[str, Any]) -> None:
    exists = conn.execute(
        sa.text(
            "SELECT 1 FROM provider_endpoints WHERE provider_id = :pid AND api_format = :fmt LIMIT 1"
        ),
        {"pid": provider_id, "fmt": _IMAGE_FORMAT},
    ).first()

    config_json = _json(_build_force_stream_config(cli.get("config")))
    if exists:
        conn.execute(
            sa.text(
                """
                UPDATE provider_endpoints
                SET api_family = 'openai',
                    endpoint_kind = 'image',
                    config = CAST(:config AS json),
                    updated_at = CURRENT_TIMESTAMP
                WHERE provider_id = :pid AND api_format = :fmt
                """
            ),
            {"pid": provider_id, "fmt": _IMAGE_FORMAT, "config": config_json},
        )
        return

    conn.execute(
        sa.text(
            """
            INSERT INTO provider_endpoints (
                id, provider_id, api_format, api_family, endpoint_kind,
                base_url, custom_path, header_rules, body_rules,
                max_retries, is_active, config, format_acceptance_config,
                proxy, created_at, updated_at
            ) VALUES (
                :id, :pid, :fmt, 'openai', 'image',
                :base_url, NULL, CAST(:header_rules AS json), CAST(:body_rules AS json),
                :max_retries, TRUE, CAST(:config AS json), NULL,
                CAST(:proxy AS jsonb), CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
            )
            """
        ),
        {
            "id": str(uuid.uuid4()),
            "pid": provider_id,
            "fmt": _IMAGE_FORMAT,
            "base_url": cli.get("base_url") or _CODEX_BASE_URL,
            "header_rules": _json(cli.get("header_rules")),
            "body_rules": _json(cli.get("body_rules")),
            "max_retries": cli.get("max_retries") or 2,
            "config": config_json,
            "proxy": _json(cli.get("proxy")),
        },
    )


def _add_image_to_key_formats(conn: sa.Connection, provider_id: str) -> None:
    rows = (
        conn.execute(
            sa.text("SELECT id, api_formats FROM provider_api_keys WHERE provider_id = :pid"),
            {"pid": provider_id},
        )
        .mappings()
        .all()
    )
    for row in rows:
        raw = row["api_formats"]
        formats: list[str] = []
        if isinstance(raw, list):
            for item in raw:
                value = str(item or "").strip().lower()
                if value and value not in formats:
                    formats.append(value)

        if _IMAGE_FORMAT in formats:
            continue

        if _CLI_FORMAT in formats:
            formats.insert(formats.index(_CLI_FORMAT) + 1, _IMAGE_FORMAT)
        else:
            formats.append(_IMAGE_FORMAT)

        conn.execute(
            sa.text(
                """
                UPDATE provider_api_keys
                SET api_formats = CAST(:fmts AS json), updated_at = CURRENT_TIMESTAMP
                WHERE id = :id
                """
            ),
            {"id": row["id"], "fmts": json.dumps(formats, ensure_ascii=False)},
        )


def upgrade() -> None:
    conn = op.get_bind()
    for provider_id in _find_codex_provider_ids(conn):
        cli = _get_cli_endpoint(conn, provider_id)
        if not cli:
            continue
        _ensure_image_endpoint(conn, provider_id, cli)
        _add_image_to_key_formats(conn, provider_id)


def downgrade() -> None:
    return
