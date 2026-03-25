from __future__ import annotations

import re

from sqlalchemy.orm import Session

from src.models.database import GlobalModel

_REASONING_SUFFIX_RE = re.compile(
    r"^(?P<base>.+)-(?P<effort>low|medium|high|xhigh)$",
    re.IGNORECASE,
)


def get_active_global_model_with_reasoning_suffix_fallback(
    db: Session,
    model_name: str | None,
) -> tuple[GlobalModel | None, str]:
    """解析活跃 GlobalModel，必要时将 reasoning 后缀模型回退到母模型。

    规则：
    1. 精确模型名优先，避免误伤真实存在的模型。
    2. 只有精确模型不存在时，才尝试将 `-low/-medium/-high/-xhigh` 回退到母模型。
    """

    normalized = str(model_name or "").strip()
    if not normalized:
        return None, normalized

    exact_model = _load_active_global_model(db, normalized)
    if exact_model is not None:
        return exact_model, normalized

    match = _REASONING_SUFFIX_RE.fullmatch(normalized)
    if not match:
        return None, normalized

    base_model_name = match.group("base").strip()
    if not base_model_name:
        return None, normalized

    base_model = _load_active_global_model(db, base_model_name)
    if base_model is None:
        return None, normalized

    return base_model, base_model_name


def _load_active_global_model(db: Session, model_name: str) -> GlobalModel | None:
    return (
        db.query(GlobalModel)
        .filter(
            GlobalModel.name == model_name,
            GlobalModel.is_active == True,
        )
        .first()
    )
