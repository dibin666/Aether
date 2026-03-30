from __future__ import annotations

from typing import Any

from fastapi import APIRouter, Depends, HTTPException, Query
from sqlalchemy.orm import Session

from src.database import get_db
from src.services.provider_keys.access_token_auto_delete import (
    get_access_token_delete_summary,
    list_access_token_delete_logs,
    restore_access_token_delete_log,
    RestoreConflictError,
)
from src.services.provider_keys.response_builder import build_key_response
from src.utils.auth_utils import require_admin

router = APIRouter(
    prefix="/api/admin/access-token-deletions",
    tags=["Admin - Access Token Deletions"],
)


def _serialize_restored_key_payload(key: Any) -> dict[str, Any]:
    if isinstance(key, dict):
        return dict(key)
    return build_key_response(key).model_dump(mode="json")


@router.get("/summary")
async def get_access_token_deletion_summary(
    db: Session = Depends(get_db),
    _: object = Depends(require_admin),
) -> dict[str, int]:
    return get_access_token_delete_summary(db)


@router.get("")
async def get_access_token_deletion_list(
    email: str | None = Query(None),
    provider_id: str | None = Query(None),
    days: int = Query(7, ge=1, le=365),
    limit: int = Query(50, ge=1, le=200),
    offset: int = Query(0, ge=0, le=5000),
    db: Session = Depends(get_db),
    _: object = Depends(require_admin),
) -> dict[str, Any]:
    return list_access_token_delete_logs(
        db,
        email=email,
        provider_id=provider_id,
        days=days,
        limit=limit,
        offset=offset,
    )


@router.post("/{log_id}/restore")
async def restore_access_token_deletion(
    log_id: str,
    db: Session = Depends(get_db),
    _: object = Depends(require_admin),
) -> dict[str, Any]:
    try:
        key = await restore_access_token_delete_log(db=db, log_id=log_id)
    except RestoreConflictError as exc:
        raise HTTPException(status_code=409, detail=str(exc)) from exc

    return {
        "message": "撤销删除成功",
        "key": _serialize_restored_key_payload(key),
    }
