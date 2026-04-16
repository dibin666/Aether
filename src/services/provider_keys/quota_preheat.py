"""
配额预热服务。

对号池中配额充足/未使用的账号发送一次最短对话请求，
使其配额窗口计时开始，从而错开各账号的配额重置时间。
"""

from __future__ import annotations

import asyncio
import json
import time
from typing import Any

import httpx
from sqlalchemy.orm import Session, defer

from src.core.crypto import crypto_service
from src.core.logger import logger
from src.core.provider_types import ProviderType, normalize_provider_type
from src.models.database import Provider, ProviderAPIKey, ProviderEndpoint
from src.services.provider.auth import get_provider_auth
from src.services.provider.pool import redis_ops as pool_redis
from src.services.provider_keys.auth_type import normalize_auth_type
from src.services.provider_keys.quota_cooldown import resolve_effective_cooldown_reason
from src.services.provider_keys.quota_reader import get_quota_reader
from src.services.proxy_node.resolver import (
    build_proxy_client_kwargs,
    resolve_effective_proxy,
)

# 支持预热的 provider 类型
PREHEAT_PROVIDER_TYPES: frozenset[str] = frozenset({
    ProviderType.CODEX,
    ProviderType.CLAUDE_CODE,
    ProviderType.KIRO,
    ProviderType.ANTIGRAVITY,
})

# 满配额阈值：usage_ratio 为 0（完全未使用）视为"满配额"
_FULL_QUOTA_THRESHOLD = 1e-6

# 并发批大小
_BATCH_SIZE = 5


def _is_full_quota_key(
    provider_type: str,
    key: ProviderAPIKey,
) -> bool:
    """判断 key 是否为"满配额"（额度剩余 100%，完全未使用）。"""
    reader = get_quota_reader(provider_type, getattr(key, "upstream_metadata", None))
    ratio = reader.usage_ratio()
    # 无配额数据（从未探测过）也视为满配额
    if ratio is None:
        return True
    return ratio < _FULL_QUOTA_THRESHOLD


def _select_endpoint_for_chat(
    provider: Provider,
    provider_type: str,
) -> ProviderEndpoint | None:
    """选择用于发送测试对话的端点。"""
    if provider_type == ProviderType.CODEX:
        for ep in provider.endpoints:
            sig = str(getattr(ep, "api_format", "") or "").strip().lower()
            if sig == "openai:cli" and ep.is_active:
                return ep
        # 回退到 openai:chat
        for ep in provider.endpoints:
            sig = str(getattr(ep, "api_format", "") or "").strip().lower()
            if sig == "openai:chat" and ep.is_active:
                return ep

    elif provider_type == ProviderType.CLAUDE_CODE:
        for ep in provider.endpoints:
            sig = str(getattr(ep, "api_format", "") or "").strip().lower()
            if sig in ("anthropic:messages", "claude:messages", "claude:chat") and ep.is_active:
                return ep

    elif provider_type == ProviderType.KIRO:
        for ep in provider.endpoints:
            sig = str(getattr(ep, "api_format", "") or "").strip().lower()
            if sig in ("anthropic:messages", "claude:messages") and ep.is_active:
                return ep

    elif provider_type == ProviderType.ANTIGRAVITY:
        for ep in provider.endpoints:
            sig = str(getattr(ep, "api_format", "") or "").strip().lower()
            if sig in ("gemini:chat", "gemini:cli") and ep.is_active:
                return ep

    # 通用回退：取第一个活跃端点
    for ep in provider.endpoints:
        if ep.is_active:
            return ep
    return None


def _build_chat_payload(
    provider_type: str,
    endpoint: ProviderEndpoint,
) -> dict[str, Any]:
    """根据 provider 类型构建最小对话请求 payload。"""
    sig = str(getattr(endpoint, "api_format", "") or "").strip().lower()

    if provider_type == ProviderType.CODEX:
        return {
            "model": "gpt-4o-mini",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 1,
            "stream": False,
        }

    if provider_type in (ProviderType.CLAUDE_CODE, ProviderType.KIRO):
        return {
            "model": "claude-3-haiku-20240307",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 1,
            "stream": False,
        }

    if provider_type == ProviderType.ANTIGRAVITY:
        if sig.startswith("gemini:"):
            return {
                "contents": [{"parts": [{"text": "hi"}]}],
                "generationConfig": {"maxOutputTokens": 1},
            }
        return {
            "model": "gemini-2.0-flash",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 1,
            "stream": False,
        }

    # 通用回退
    return {
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1,
        "stream": False,
    }


def _build_chat_url(
    endpoint: ProviderEndpoint,
    provider_type: str,
    auth_info: Any | None,
) -> str:
    """构建用于测试对话的上游 URL。"""
    from src.services.provider.transport import build_provider_url

    try:
        return build_provider_url(
            endpoint,
            is_stream=False,
            key=None,
            decrypted_auth_config=(
                auth_info.decrypted_auth_config if auth_info else None
            ),
        )
    except Exception:
        # 回退：使用 base_url + 默认路径
        base = str(getattr(endpoint, "base_url", "") or "").rstrip("/")
        sig = str(getattr(endpoint, "api_format", "") or "").strip().lower()
        if sig.startswith("anthropic:") or sig.startswith("claude:"):
            return f"{base}/v1/messages"
        if sig.startswith("gemini:"):
            return f"{base}/v1/models/gemini-2.0-flash:generateContent"
        return f"{base}/v1/chat/completions"


async def _preheat_single_key(
    *,
    provider: Provider,
    provider_type: str,
    key: ProviderAPIKey,
    endpoint: ProviderEndpoint,
) -> dict[str, Any]:
    """对单个 key 发送一次最短对话请求。"""
    key_id = str(key.id)
    key_name = str(key.name or key_id[:8])

    try:
        # 获取认证信息
        auth_info = await get_provider_auth(endpoint, key)

        # 构建请求头
        headers: dict[str, str] = {
            "Accept": "application/json",
            "Content-Type": "application/json",
        }
        if auth_info:
            headers[auth_info.auth_header] = auth_info.auth_value
        else:
            decrypted_key = crypto_service.decrypt(key.api_key)
            headers["Authorization"] = f"Bearer {decrypted_key}"

        # Codex OAuth: 添加 account_id 头
        if provider_type == ProviderType.CODEX:
            auth_type = normalize_auth_type(getattr(key, "auth_type", "api_key"))
            if auth_type == "oauth" and key.auth_config:
                try:
                    decrypted_config = crypto_service.decrypt(key.auth_config)
                    auth_config_data = json.loads(decrypted_config)
                    if isinstance(auth_config_data, dict):
                        plan_type = str(
                            auth_config_data.get("plan_type", "")
                        ).strip().lower()
                        account_id = auth_config_data.get("account_id")
                        if (
                            isinstance(account_id, str)
                            and account_id.strip()
                            and plan_type != "free"
                        ):
                            headers["chatgpt-account-id"] = account_id.strip()
                except Exception:
                    pass

        # 解析代理配置
        effective_proxy = resolve_effective_proxy(
            getattr(provider, "proxy", None),
            getattr(key, "proxy", None),
        )

        # 构建请求 URL 和 payload
        url = _build_chat_url(endpoint, provider_type, auth_info)
        payload = _build_chat_payload(provider_type, endpoint)

        # 发送请求
        async with httpx.AsyncClient(
            **build_proxy_client_kwargs(effective_proxy, timeout=30.0)
        ) as client:
            response = await client.post(url, json=payload, headers=headers)

        status_code = response.status_code
        if 200 <= status_code < 300:
            logger.info(
                "[PREHEAT] key={} ({}) 预热成功: status={}",
                key_id[:8],
                key_name,
                status_code,
            )
            return {
                "key_id": key_id,
                "key_name": key_name,
                "status": "success",
                "message": f"预热成功 (HTTP {status_code})",
            }
        else:
            # 非 2xx 也算"预热生效"——只要发送了请求就消耗了配额
            detail = ""
            try:
                body = response.json()
                if isinstance(body, dict):
                    err = body.get("error")
                    if isinstance(err, dict):
                        detail = str(err.get("message", ""))[:200]
                    elif isinstance(err, str):
                        detail = err[:200]
            except Exception:
                detail = str(response.text or "")[:200]

            logger.warning(
                "[PREHEAT] key={} ({}) 预热完成但上游返回非 2xx: status={}, detail={}",
                key_id[:8],
                key_name,
                status_code,
                detail,
            )
            return {
                "key_id": key_id,
                "key_name": key_name,
                "status": "warning",
                "message": f"HTTP {status_code}: {detail}" if detail else f"HTTP {status_code}",
            }

    except Exception as exc:
        error_msg = str(exc) or type(exc).__name__
        logger.error(
            "[PREHEAT] key={} ({}) 预热失败: {}",
            key_id[:8],
            key_name,
            error_msg,
        )
        return {
            "key_id": key_id,
            "key_name": key_name,
            "status": "error",
            "message": error_msg[:300],
        }


async def preheat_full_quota_keys(
    db: Session,
    provider_id: str,
) -> dict[str, Any]:
    """对指定 Provider 下所有满配额的 key 执行预热。

    Returns:
        {
            "total": int,       # 满配额账号总数
            "success": int,     # 预热成功数
            "failed": int,      # 预热失败数
            "skipped": int,     # 跳过数（不满足条件）
            "details": [...]    # 每个 key 的详细结果
        }
    """
    from src.core.exceptions import InvalidRequestException, NotFoundException

    provider = db.query(Provider).filter(Provider.id == provider_id).first()
    if not provider:
        raise NotFoundException(f"Provider {provider_id} 不存在", "provider")

    provider_type = normalize_provider_type(getattr(provider, "provider_type", ""))
    if provider_type not in PREHEAT_PROVIDER_TYPES:
        raise InvalidRequestException(
            f"Provider 类型 {provider_type} 不支持配额预热，"
            f"仅支持: {', '.join(sorted(PREHEAT_PROVIDER_TYPES))}"
        )

    # 选择端点
    endpoint = _select_endpoint_for_chat(provider, provider_type)
    if endpoint is None:
        raise InvalidRequestException("找不到适用的端点用于发送测试对话")

    # 查询所有活跃 key
    keys = (
        db.query(ProviderAPIKey)
        .options(
            defer(ProviderAPIKey.health_by_format),
            defer(ProviderAPIKey.circuit_breaker_by_format),
            defer(ProviderAPIKey.adjustment_history),
            defer(ProviderAPIKey.utilization_samples),
        )
        .filter(
            ProviderAPIKey.provider_id == provider_id,
            ProviderAPIKey.is_active.is_(True),
        )
        .all()
    )

    if not keys:
        return {
            "total": 0,
            "success": 0,
            "failed": 0,
            "skipped": 0,
            "details": [],
        }

    # 筛选满配额 key（同时排除有冷却和被封禁的）
    pid = str(provider.id)
    redis_client = None
    try:
        from src.clients.redis_client import get_redis_client
        redis_client = await get_redis_client(require_redis=False)
    except Exception:
        pass

    eligible_keys: list[ProviderAPIKey] = []
    skipped_details: list[dict[str, Any]] = []

    for key in keys:
        key_id = str(key.id)
        key_name = str(key.name or key_id[:8])

        # 检查冷却状态
        redis_reason = None
        if redis_client:
            try:
                redis_reason = await pool_redis.get_cooldown(pid, key_id)
            except Exception:
                pass

        cooldown = resolve_effective_cooldown_reason(
            provider_type=provider_type,
            key=key,
            redis_reason=redis_reason,
        )
        if cooldown:
            skipped_details.append({
                "key_id": key_id,
                "key_name": key_name,
                "status": "skipped",
                "message": f"已有冷却: {cooldown}",
            })
            continue

        # 检查是否满配额
        if not _is_full_quota_key(provider_type, key):
            skipped_details.append({
                "key_id": key_id,
                "key_name": key_name,
                "status": "skipped",
                "message": "配额已有使用记录，非满配额",
            })
            continue

        eligible_keys.append(key)

    if not eligible_keys:
        return {
            "total": 0,
            "success": 0,
            "failed": 0,
            "skipped": len(skipped_details),
            "details": skipped_details,
        }

    # 分批并发执行预热
    preheat_results: list[dict[str, Any]] = []
    for i in range(0, len(eligible_keys), _BATCH_SIZE):
        batch = eligible_keys[i : i + _BATCH_SIZE]
        tasks = [
            _preheat_single_key(
                provider=provider,
                provider_type=provider_type,
                key=key,
                endpoint=endpoint,
            )
            for key in batch
        ]
        batch_results = await asyncio.gather(*tasks)
        preheat_results.extend(batch_results)

    # 统计
    success_count = sum(1 for r in preheat_results if r["status"] == "success")
    warning_count = sum(1 for r in preheat_results if r["status"] == "warning")
    failed_count = sum(1 for r in preheat_results if r["status"] == "error")

    all_details = preheat_results + skipped_details

    logger.info(
        "[PREHEAT] Provider {} ({}) 预热完成: eligible={}, success={}, warning={}, failed={}, skipped={}",
        provider_id[:8],
        provider_type,
        len(eligible_keys),
        success_count,
        warning_count,
        failed_count,
        len(skipped_details),
    )

    return {
        "total": len(eligible_keys),
        "success": success_count + warning_count,
        "failed": failed_count,
        "skipped": len(skipped_details),
        "details": all_details,
    }
