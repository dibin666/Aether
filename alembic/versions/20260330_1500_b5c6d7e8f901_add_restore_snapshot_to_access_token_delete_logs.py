"""add restore snapshot to access token delete logs

Revision ID: b5c6d7e8f901
Revises: a4b5c6d7e8f9
Create Date: 2026-03-30 15:00:00.000000+00:00
"""

from __future__ import annotations

from collections.abc import Sequence

import sqlalchemy as sa

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "b5c6d7e8f901"
down_revision: str | None = "a4b5c6d7e8f9"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    op.add_column(
        "access_token_delete_logs",
        sa.Column("snapshot_api_key", sa.Text(), nullable=True),
    )
    op.add_column(
        "access_token_delete_logs",
        sa.Column("snapshot_auth_config", sa.Text(), nullable=True),
    )
    op.add_column(
        "access_token_delete_logs",
        sa.Column("snapshot_payload", sa.JSON(), nullable=True),
    )
    op.add_column(
        "access_token_delete_logs",
        sa.Column(
            "restore_status",
            sa.String(length=20),
            nullable=False,
            server_default="legacy",
        ),
    )
    op.add_column(
        "access_token_delete_logs",
        sa.Column("restored_key_id", sa.String(length=36), nullable=True),
    )
    op.add_column(
        "access_token_delete_logs",
        sa.Column("restored_at", sa.DateTime(timezone=True), nullable=True),
    )
    op.add_column(
        "access_token_delete_logs",
        sa.Column("restore_error", sa.Text(), nullable=True),
    )
    op.create_index(
        "ix_access_token_delete_logs_restore_status",
        "access_token_delete_logs",
        ["restore_status"],
    )
    op.create_index(
        "ix_access_token_delete_logs_restored_key_id",
        "access_token_delete_logs",
        ["restored_key_id"],
    )


def downgrade() -> None:
    op.drop_index(
        "ix_access_token_delete_logs_restored_key_id",
        table_name="access_token_delete_logs",
    )
    op.drop_index(
        "ix_access_token_delete_logs_restore_status",
        table_name="access_token_delete_logs",
    )
    op.drop_column("access_token_delete_logs", "restore_error")
    op.drop_column("access_token_delete_logs", "restored_at")
    op.drop_column("access_token_delete_logs", "restored_key_id")
    op.drop_column("access_token_delete_logs", "restore_status")
    op.drop_column("access_token_delete_logs", "snapshot_payload")
    op.drop_column("access_token_delete_logs", "snapshot_auth_config")
    op.drop_column("access_token_delete_logs", "snapshot_api_key")
