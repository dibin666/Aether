#!/bin/bash
# 智能构建脚本 - 自动检测代码变化并重建镜像
#
# 用法:
#   构建/更新镜像:  ./deploy.sh
#   强制全部重建:  ./deploy.sh --force
#   追加自定义 tag: ./deploy.sh --tag v20260427

set -euo pipefail
cd "$(dirname "$0")"

# 缓存文件
CODE_HASH_FILE=".code-hash"
IMAGE_NAME="aether-app"
DEFAULT_IMAGE_TAG="latest"
PRIMARY_IMAGE_REF="${IMAGE_NAME}:${DEFAULT_IMAGE_TAG}"
CUSTOM_IMAGE_TAG=""

usage() {
    cat <<'EOF'
Usage: ./deploy.sh [options]

Options:
  --force, -f             强制重建镜像
  --tag, -t TAG           额外打自定义 tag（始终保留 latest）
  -h, --help              显示帮助
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

apply_custom_tag() {
    if [ -z "$CUSTOM_IMAGE_TAG" ] || [ "$CUSTOM_IMAGE_TAG" = "$DEFAULT_IMAGE_TAG" ]; then
        return
    fi

    local custom_image_ref="${IMAGE_NAME}:${CUSTOM_IMAGE_TAG}"
    echo ">>> Tagging image as ${custom_image_ref}..."
    docker tag "$PRIMARY_IMAGE_REF" "$custom_image_ref"
}

print_result() {
    echo ">>> Done!"
    echo ">>> Built image: ${PRIMARY_IMAGE_REF}"
    if [ -n "$CUSTOM_IMAGE_TAG" ] && [ "$CUSTOM_IMAGE_TAG" != "$DEFAULT_IMAGE_TAG" ]; then
        echo ">>> Additional tag: ${IMAGE_NAME}:${CUSTOM_IMAGE_TAG}"
    fi
    echo ">>> To start containers, run: docker compose -f docker-compose.build.yml up -d --no-build"
    docker image ls "$IMAGE_NAME"
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

# 计算代码文件的哈希值
calc_code_hash() {
    {
        cat Dockerfile.app.local 2>/dev/null
        cat Cargo.toml Cargo.lock 2>/dev/null
        find frontend/src -type f \( -name "*.vue" -o -name "*.ts" -o -name "*.tsx" -o -name "*.js" \) 2>/dev/null | sort | xargs cat 2>/dev/null
        find apps -type f \( -name "*.rs" -o -name "Cargo.toml" \) 2>/dev/null | sort | xargs cat 2>/dev/null
        find crates -type f \( -name "*.rs" -o -name "*.sql" -o -name "Cargo.toml" \) 2>/dev/null | sort | xargs cat 2>/dev/null
    } | md5sum | cut -d' ' -f1
}

# 检查代码是否变化
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

save_code_hash() { calc_code_hash > "$CODE_HASH_FILE"; }

# 构建应用镜像
build_app() {
    echo ">>> Building app image (${PRIMARY_IMAGE_REF})..."
    docker build --pull=false -f Dockerfile.app.local -t "$PRIMARY_IMAGE_REF" .
    apply_custom_tag
    save_code_hash
}

if [ "$FORCE_REBUILD_ALL" = true ]; then
    echo ">>> Force rebuilding app image..."
    build_app
    docker image prune -f >/dev/null 2>&1 || true
    print_result
    exit 0
fi

if ! docker image inspect "$PRIMARY_IMAGE_REF" >/dev/null 2>&1; then
    echo ">>> App image not found, building..."
    build_app
elif check_code_changed; then
    echo ">>> Code changed, rebuilding app image..."
    build_app
else
    echo ">>> Code unchanged. Existing image is up to date."
    apply_custom_tag
fi

docker image prune -f >/dev/null 2>&1 || true

print_result