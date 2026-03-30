"""add access token delete logs

Revision ID: a4b5c6d7e8f9
Revises: c3d4e5f6a7b8
Create Date: 2026-03-30 11:00:00.000000+00:00
"""

from __future__ import annotations

from collections.abc import Sequence

import sqlalchemy as sa

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "a4b5c6d7e8f9"
down_revision: str | None = "c3d4e5f6a7b8"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    op.create_table(
        "access_token_delete_logs",
        sa.Column("id", sa.String(length=36), primary_key=True),
        sa.Column("deleted_key_id", sa.String(length=36), nullable=False),
        sa.Column("provider_id", sa.String(length=36), nullable=False),
        sa.Column("provider_name", sa.String(length=255), nullable=True),
        sa.Column("key_name", sa.String(length=255), nullable=True),
        sa.Column("oauth_email", sa.String(length=255), nullable=True),
        sa.Column("provider_type", sa.String(length=50), nullable=False),
        sa.Column("auth_type", sa.String(length=20), nullable=False),
        sa.Column("trigger_status_code", sa.Integer(), nullable=False),
        sa.Column("endpoint_sig", sa.String(length=100), nullable=True),
        sa.Column("proxy_node_id", sa.String(length=36), nullable=True),
        sa.Column("proxy_node_name", sa.String(length=255), nullable=True),
        sa.Column("request_id", sa.String(length=255), nullable=True),
        sa.Column("error_message", sa.Text(), nullable=True),
        sa.Column("raw_error_excerpt", sa.Text(), nullable=True),
        sa.Column("deleted_by", sa.String(length=64), nullable=False),
        sa.Column("deleted_at", sa.DateTime(timezone=True), nullable=False),
        sa.UniqueConstraint("deleted_key_id", name="uq_access_token_delete_logs_deleted_key_id"),
    )
    op.create_index(
        "ix_access_token_delete_logs_provider_id",
        "access_token_delete_logs",
        ["provider_id"],
    )
    op.create_index(
        "ix_access_token_delete_logs_oauth_email",
        "access_token_delete_logs",
        ["oauth_email"],
    )
    op.create_index(
        "ix_access_token_delete_logs_deleted_at",
        "access_token_delete_logs",
        ["deleted_at"],
    )


def downgrade() -> None:
    op.drop_index("ix_access_token_delete_logs_deleted_at", table_name="access_token_delete_logs")
    op.drop_index(
        "ix_access_token_delete_logs_oauth_email",
        table_name="access_token_delete_logs",
    )
    op.drop_index(
        "ix_access_token_delete_logs_provider_id",
        table_name="access_token_delete_logs",
    )
    op.drop_table("access_token_delete_logs")
