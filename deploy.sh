#!/bin/bash
# 智能构建脚本 - 自动检测代码变化并重建镜像，保留本地镜像追加 tag 能力
#
# 用法:
#   构建/更新镜像:  ./deploy.sh
#   强制重建镜像:  ./deploy.sh --force
#   追加自定义 tag: ./deploy.sh --tag v20260427

set -euo pipefail
cd "$(dirname "$0")"

IMAGE_NAME="${IMAGE_NAME:-aether-app}"
DEFAULT_IMAGE_TAG="${DEFAULT_IMAGE_TAG:-latest}"
LOCAL_APP_IMAGE="${LOCAL_APP_IMAGE:-${IMAGE_NAME}:${DEFAULT_IMAGE_TAG}}"
CUSTOM_IMAGE_TAG=""
export LOCAL_APP_IMAGE

# 缓存文件
CODE_HASH_FILE=".code-hash"

usage() {
    cat <<'EOF'
Usage: ./deploy.sh [options]

Options:
  --force, -f             强制重建镜像
  --tag, -t TAG           额外打自定义 tag（始终保留 latest）
  -h, --help              显示帮助

Environment:
  LOCAL_APP_IMAGE          本地构建镜像名，默认 aether-app:latest
EOF
}

validate_tag() {
    local tag="$1"
    if [[ ! "$tag" =~ ^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$ ]]; then
        echo "Invalid tag: ${tag}"
        echo "Tag must match ^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$"
        exit 1
    fi
}

custom_image_ref() {
    local image_repo="$LOCAL_APP_IMAGE"
    if [[ "$image_repo" == *:* ]]; then
        image_repo="${image_repo%:*}"
    fi
    printf '%s:%s' "$image_repo" "$CUSTOM_IMAGE_TAG"
}

apply_custom_tag() {
    if [ -z "$CUSTOM_IMAGE_TAG" ] || [ "$CUSTOM_IMAGE_TAG" = "$DEFAULT_IMAGE_TAG" ]; then
        return
    fi

    local custom_ref
    custom_ref="$(custom_image_ref)"
    echo ">>> Tagging image as ${custom_ref}..."
    docker tag "$LOCAL_APP_IMAGE" "$custom_ref"
}

print_result() {
    echo ">>> Done!"
    echo ">>> Built image: ${LOCAL_APP_IMAGE}"
    if [ -n "$CUSTOM_IMAGE_TAG" ] && [ "$CUSTOM_IMAGE_TAG" != "$DEFAULT_IMAGE_TAG" ]; then
        echo ">>> Additional tag: $(custom_image_ref)"
    fi
}

FORCE_REBUILD_ALL=false

while [ $# -gt 0 ]; do
    case "$1" in
        --force|-f)
            FORCE_REBUILD_ALL=true
            shift
            ;;
        --tag|-t)
            if [ $# -lt 2 ]; then
                echo "Missing value for $1"
                usage
                exit 1
            fi
            CUSTOM_IMAGE_TAG="$2"
            validate_tag "$CUSTOM_IMAGE_TAG"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1"
            usage
            exit 1
            ;;
    esac
done

require_file() {
    if [ ! -f "$1" ]; then
        echo "Required file not found: $1"
        exit 1
    fi
}

hash_stream() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum | cut -d' ' -f1
    else
        shasum -a 256 | cut -d' ' -f1
    fi
}

emit_file_for_hash() {
    local path="$1"
    [ -f "$path" ] || return 0
    printf '\n>>> %s\n' "$path"
    cat "$path"
}

emit_tree_for_hash() {
    local root="$1"
    [ -d "$root" ] || return 0
    find "$root" -type f \
        ! -path '*/node_modules/*' \
        ! -path '*/target/*' \
        ! -path '*/dist/*' \
        ! -path '*/.mypy_cache/*' \
        ! -path '*/.vite/*' \
        2>/dev/null | sort | while IFS= read -r path; do
        emit_file_for_hash "$path"
    done
}

# 计算代码文件的哈希值
calc_code_hash() {
    {
        for file in \
            Dockerfile.app.local \
            docker-compose.yml \
            docker-compose.local.yml \
            .dockerignore \
            Cargo.toml \
            Cargo.lock \
            rust-toolchain.toml \
            frontend/package.json \
            frontend/package-lock.json \
            frontend/index.html \
            frontend/vite.config.ts \
            frontend/tsconfig.json \
            frontend/tsconfig.app.json \
            frontend/tsconfig.node.json \
            frontend/postcss.config.js \
            frontend/tailwind.config.js; do
            emit_file_for_hash "$file"
        done

        for dir in frontend/src frontend/public apps crates; do
            emit_tree_for_hash "$dir"
        done
    } | hash_stream
}

# 检查代码是否变化
check_code_changed() {
    local current_hash
    current_hash=$(calc_code_hash)
    if [ -f "$CODE_HASH_FILE" ]; then
        local saved_hash
        saved_hash=$(cat "$CODE_HASH_FILE")
        if [ "$current_hash" = "$saved_hash" ]; then
            return 1
        fi
    fi
    return 0
}

save_code_hash() { calc_code_hash > "$CODE_HASH_FILE"; }

# 构建应用镜像
build_app() {
    require_file Dockerfile.app.local
    echo ">>> Building app image: $LOCAL_APP_IMAGE"
    DOCKER_BUILDKIT="${DOCKER_BUILDKIT:-1}" docker build --pull=false -f Dockerfile.app.local -t "$LOCAL_APP_IMAGE" .
    apply_custom_tag
    save_code_hash
}

# 强制全部重建
if [ "$FORCE_REBUILD_ALL" = true ]; then
    echo ">>> Force rebuilding app image..."
    build_app
    print_result
    exit 0
fi

# 检查代码是否变化
if ! docker image inspect "$LOCAL_APP_IMAGE" >/dev/null 2>&1; then
    echo ">>> App image not found, building..."
    build_app
elif check_code_changed; then
    echo ">>> Code changed, rebuilding app image..."
    build_app
else
    echo ">>> Code unchanged. Existing image is up to date."
    apply_custom_tag
fi

print_result
