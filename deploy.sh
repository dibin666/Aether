#!/bin/bash
# 本地镜像构建脚本 - 自动检测依赖/代码变化并构建镜像
#
# 用法:
#   构建镜像:      ./deploy.sh                    (自动检测变化并构建镜像)
#   指定镜像标签:  ./deploy.sh --tag test-1      (构建 aether-*:test-1)
#   指定 Hub 版本: ./deploy.sh --hub-tag hub-v0.1.0
#   更新 Hub:      ./deploy.sh --update-hub       (刷新 Hub 版本并重建 app)
#   GitHub 镜像:   ./deploy.sh --mirror https://ghfast.top
#   强制重建:      ./deploy.sh --rebuild-base
#   强制全部重建:  ./deploy.sh --force

set -euo pipefail
cd "$(dirname "$0")"

# 缓存文件
HASH_FILE=".deps-hash"
CODE_HASH_FILE=".code-hash"

# Hub release 配置
GITHUB_REPO="fawney19/Aether"
HUB_TAG_STATE_FILE=".hub-tag"

usage() {
    cat <<'EOF'
Usage: ./deploy.sh [options]

Options:
  --tag <tag>             指定本地镜像 tag（例如 test-1）
  --hub-tag <hub-vX.Y.Z>  指定 Hub Release tag（例如 hub-v0.1.0）
  --update-hub            强制刷新 Hub 版本标记并重建 app 镜像
  --mirror <url>          GitHub 下载镜像（例如 https://ghfast.top）
  --rebuild-base, -r      仅重建 base 镜像
  --force, -f             强制重建全部（hub/base/app）
  -h, --help              显示帮助
EOF
}

FORCE_REBUILD_ALL=false
REBUILD_BASE_ONLY=false
FORCE_UPDATE_HUB=false
HUB_TAG="${HUB_TAG:-}"
GITHUB_MIRROR="${GITHUB_MIRROR:-}"
IMAGE_TAG="${IMAGE_TAG:-latest}"
RESOLVED_HUB_TAG=""

while [ $# -gt 0 ]; do
    case "$1" in
        --tag)
            if [ $# -lt 2 ]; then
                echo "❌ --tag 需要一个值，例如 test-1"
                exit 1
            fi
            IMAGE_TAG="$2"
            shift 2
            ;;
        --hub-tag)
            if [ $# -lt 2 ]; then
                echo "❌ --hub-tag 需要一个值，例如 hub-v0.1.0"
                exit 1
            fi
            HUB_TAG="$2"
            shift 2
            ;;
        --update-hub)
            FORCE_UPDATE_HUB=true
            shift
            ;;
        --mirror)
            if [ $# -lt 2 ]; then
                echo "ERROR: --mirror needs a URL, e.g. https://ghfast.top"
                exit 1
            fi
            GITHUB_MIRROR="$2"
            shift 2
            ;;
        --rebuild-base|-r)
            REBUILD_BASE_ONLY=true
            shift
            ;;
        --force|-f)
            FORCE_REBUILD_ALL=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "❌ 未知参数: $1"
            usage
            exit 1
            ;;
    esac
done

case "$IMAGE_TAG" in
    ''|*[!A-Za-z0-9._-]*)
        echo "❌ --tag 仅支持字母、数字、点、下划线和短横线"
        exit 1
        ;;
esac

BASE_IMAGE_REPO="aether-base"
APP_IMAGE_REPO="aether-app"
LATEST_BASE_IMAGE="${BASE_IMAGE_REPO}:latest"
BASE_IMAGE="${BASE_IMAGE_REPO}:${IMAGE_TAG}"
APP_IMAGE="${APP_IMAGE_REPO}:${IMAGE_TAG}"

if [ -n "$HUB_TAG" ]; then
    case "$HUB_TAG" in
        hub-v*) ;;
        *) echo "❌ --hub-tag 格式应为 hub-vX.Y.Z，例如 hub-v0.1.0"; exit 1 ;;
    esac
fi

# 提取 pyproject.toml 中会影响运行时依赖安装的字段指纹（纯 shell，无需 Python）
# 用 sed 提取 dependencies / requires 数组块和单值字段，排序后输出稳定文本
pyproject_deps_fingerprint() {
    local file="pyproject.toml"
    # 提取 "key = [..." 多行数组块（从 key 行到 ] 行）
    extract_array() {
        sed -n "/^$1[[:space:]]*=[[:space:]]*\[/,/\]/p" "$file" | grep '"' | sed 's/.*"\(.*\)".*/\1/' | sort
    }
    # 提取 "key = "value"" 单行值
    extract_value() {
        grep -m1 "^$1[[:space:]]*=" "$file" 2>/dev/null | sed 's/.*"\(.*\)".*/\1/'
    }
    {
        echo "requires-python=$(extract_value requires-python)"
        echo "build-backend=$(extract_value build-backend)"
        echo "dependencies:"
        extract_array dependencies
        echo "build-requires:"
        extract_array requires
    }
}

# 计算依赖文件的哈希值（包含 Dockerfile.base.local）
calc_deps_hash() {
    {
        cat Dockerfile.base.local 2>/dev/null
        pyproject_deps_fingerprint
        # 前端依赖以 lock 为准（避免仅改 scripts/version 触发 base 重建）
        cat frontend/package-lock.json 2>/dev/null
    } | md5sum | cut -d' ' -f1
}

# 计算代码文件的哈希值（包含 Dockerfile.app.local）
calc_code_hash() {
    {
        cat Dockerfile.app.local 2>/dev/null
        cat alembic.ini 2>/dev/null
        cat gunicorn_conf.py 2>/dev/null
        cat entrypoint.sh 2>/dev/null
        find src -type f -name "*.py" 2>/dev/null | sort | xargs cat 2>/dev/null
        find alembic -type f -name "*.py" 2>/dev/null | sort | xargs cat 2>/dev/null
        find frontend/src -type f \( -name "*.vue" -o -name "*.ts" -o -name "*.tsx" -o -name "*.js" \) 2>/dev/null | sort | xargs cat 2>/dev/null
    } | md5sum | cut -d' ' -f1
}

# 获取最新 hub release tag
# 支持 GITHUB_TOKEN 环境变量以避免未认证 API 限流（60 次/小时 -> 5000 次/小时）
get_latest_hub_tag() {
    local auth_args=()
    if [ -n "${GITHUB_TOKEN:-}" ]; then
        auth_args=(-H "Authorization: token ${GITHUB_TOKEN}")
    fi
    curl -sL "${auth_args[@]}" "https://api.github.com/repos/$GITHUB_REPO/releases" | \
        python3 -c "
import json, sys
releases = json.load(sys.stdin)
for r in releases:
    tag = r.get('tag_name', '')
    if tag.startswith('hub-v') and not r.get('draft') and not r.get('prerelease'):
        print(tag)
        break
" 2>/dev/null
}

# 解析当前应使用的 Hub release tag（优先使用指定值，否则拉取最新）
resolve_hub_tag() {
    local requested_tag="${1:-}"
    local latest_tag

    if [ -n "$requested_tag" ]; then
        echo "$requested_tag"
        return 0
    fi

    latest_tag="$(get_latest_hub_tag || true)"
    if [ -n "$latest_tag" ]; then
        echo "$latest_tag"
        return 0
    fi

    if [ -f "$HUB_TAG_STATE_FILE" ]; then
        echo "⚠️ 无法查询最新 Hub 版本，回退使用本地记录: $(cat "$HUB_TAG_STATE_FILE")" >&2
        cat "$HUB_TAG_STATE_FILE"
        return 0
    fi

    echo "❌ 无法获取 Hub Release tag，请检查网络或手动指定 --hub-tag" >&2
    exit 1
}

# 确保本次构建的 Hub tag 已解析（默认追踪最新 release，也可通过 --hub-tag 固定版本）
ensure_hub_tag() {
    local requested_tag="${1:-}"
    RESOLVED_HUB_TAG="$(resolve_hub_tag "$requested_tag")"

    if [ -f "$HUB_TAG_STATE_FILE" ] && [ "$(cat "$HUB_TAG_STATE_FILE")" = "$RESOLVED_HUB_TAG" ]; then
        echo ">>> Hub 版本未变化: $RESOLVED_HUB_TAG"
        return 1
    fi

    echo "$RESOLVED_HUB_TAG" > "$HUB_TAG_STATE_FILE"
    echo ">>> 使用 Hub 版本: $RESOLVED_HUB_TAG"
    return 0
}

# 检查依赖是否变化
check_deps_changed() {
    local current_hash=$(calc_deps_hash)
    if [ -f "$HASH_FILE" ]; then
        local saved_hash=$(cat "$HASH_FILE")
        if [ "$current_hash" = "$saved_hash" ]; then
            return 1
        fi
    fi
    return 0
}

# 检查应用镜像相关文件是否变化
check_code_changed() {
    local current_hash=$(calc_code_hash)
    if [ -f "$CODE_HASH_FILE" ]; then
        local saved_hash=$(cat "$CODE_HASH_FILE")
        if [ "$current_hash" = "$saved_hash" ]; then
            return 1
        fi
    fi
    return 0
}

# 保存哈希
save_deps_hash() { calc_deps_hash > "$HASH_FILE"; }
save_code_hash() { calc_code_hash > "$CODE_HASH_FILE"; }

# 构建基础镜像
build_base() {
    echo ">>> Building base image (dependencies): $BASE_IMAGE"
    docker build --pull=false -f Dockerfile.base.local -t "$BASE_IMAGE" .
    if [ "$BASE_IMAGE" != "$LATEST_BASE_IMAGE" ]; then
        docker tag "$BASE_IMAGE" "$LATEST_BASE_IMAGE"
    fi
    save_deps_hash
}


# 生成版本文件
generate_version_file() {
    # 从 git 获取版本号
    local version
    version=$(git describe --tags --always 2>/dev/null | sed 's/^v//')
    if [ -z "$version" ]; then
        version="unknown"
    fi
    echo ">>> Generating version file: $version"
    cat > src/_version.py << EOF
# Auto-generated by deploy.sh - do not edit
__version__ = '$version'
__version_tuple__ = tuple(int(x) for x in '$version'.split('-')[0].split('.') if x.isdigit())
version = __version__
version_tuple = __version_tuple__
EOF
}

# 构建应用镜像
build_app() {
    echo ">>> Building app image (code only): $APP_IMAGE"
    if [ -z "${RESOLVED_HUB_TAG:-}" ]; then
        echo ">>> RESOLVED_HUB_TAG 为空，无法构建 app 镜像"
        exit 1
    fi
    if [ "$BASE_IMAGE" != "$LATEST_BASE_IMAGE" ]; then
        docker tag "$BASE_IMAGE" "$LATEST_BASE_IMAGE"
    fi
    echo ">>> Build args: HUB_TAG=$RESOLVED_HUB_TAG"
    generate_version_file
    local token_args=()
    if [ -n "${GITHUB_TOKEN:-}" ]; then
        token_args=(--build-arg "GITHUB_TOKEN=${GITHUB_TOKEN}")
    fi
    local mirror_args=()
    if [ -n "${GITHUB_MIRROR:-}" ]; then
        mirror_args=(--build-arg "GITHUB_MIRROR=${GITHUB_MIRROR}")
    fi
    docker build --pull=false \
        --build-arg HUB_RELEASE_REPO="$GITHUB_REPO" \
        --build-arg HUB_TAG="$RESOLVED_HUB_TAG" \
        "${token_args[@]}" \
        "${mirror_args[@]}" \
        -f Dockerfile.app.local \
        -t "$APP_IMAGE" .
    save_code_hash
}

print_built_images() {
    echo ">>> 本地镜像构建完成"
    for image in "$BASE_IMAGE" "$APP_IMAGE"; do
        if docker image inspect "$image" >/dev/null 2>&1; then
            echo "$image"
        fi
    done
}

# 强制全部重建
if [ "$FORCE_REBUILD_ALL" = true ]; then
    echo ">>> Force rebuilding everything..."
    if [ "$FORCE_UPDATE_HUB" = true ]; then
        rm -f "$HUB_TAG_STATE_FILE"
    fi
    ensure_hub_tag "$HUB_TAG" || true
    build_base
    build_app
    print_built_images
    exit 0
fi

# 强制重建基础镜像
if [ "$REBUILD_BASE_ONLY" = true ]; then
    build_base
    print_built_images
    exit 0
fi

# 更新 Hub 版本标记后继续正常构建流程
if [ "$FORCE_UPDATE_HUB" = true ]; then
    rm -f "$HUB_TAG_STATE_FILE"
fi

BASE_REBUILT=false
HUB_UPDATED=false

# 检查基础镜像是否存在，或依赖是否变化
if ! docker image inspect "$BASE_IMAGE" >/dev/null 2>&1; then
    echo ">>> Base image not found, building..."
    build_base
    BASE_REBUILT=true
elif check_deps_changed; then
    echo ">>> Dependencies changed, rebuilding base image..."
    build_base
    BASE_REBUILT=true
else
    echo ">>> Dependencies unchanged."
fi

# 解析/检查 Hub 版本（构建时由 Dockerfile 从 GitHub Release 下载）
if ensure_hub_tag "$HUB_TAG"; then
    HUB_UPDATED=true
else
    echo ">>> Hub version unchanged."
fi

# 检查应用镜像相关文件是否变化，或者 base / hub 变化了
if ! docker image inspect "$APP_IMAGE" >/dev/null 2>&1; then
    echo ">>> App image not found, building..."
    build_app
elif [ "$BASE_REBUILT" = true ]; then
    echo ">>> Base image rebuilt, rebuilding app image..."
    build_app
elif [ "$HUB_UPDATED" = true ]; then
    echo ">>> Hub version updated, rebuilding app image..."
    build_app
elif check_code_changed; then
    echo ">>> Code changed, rebuilding app image..."
    build_app
else
    echo ">>> Code unchanged."
fi

print_built_images
