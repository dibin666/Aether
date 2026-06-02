#!/bin/bash
# 智能构建脚本 - 自动检测代码变化并重建镜像，保留本地镜像追加 tag 能力
#
# 用法:
#   构建/更新镜像:  ./deploy.sh
#   强制重建镜像:  ./deploy.sh --force
#   追加自定义 tag: ./deploy.sh --tag v20260427
#   下载上游 tunnel: AETHER_TUNNEL_MODE=release AETHER_TUNNEL_RELEASE_TAG=tunnel-v0.3.13 ./deploy.sh

set -euo pipefail
cd "$(dirname "$0")"

IMAGE_NAME="${IMAGE_NAME:-aether-app}"
DEFAULT_IMAGE_TAG="${DEFAULT_IMAGE_TAG:-latest}"
LOCAL_APP_IMAGE="${LOCAL_APP_IMAGE:-${IMAGE_NAME}:${DEFAULT_IMAGE_TAG}}"
CUSTOM_IMAGE_TAG=""
AETHER_TUNNEL_MODE="${AETHER_TUNNEL_MODE:-source}"
AETHER_TUNNEL_RELEASE_REPO="${AETHER_TUNNEL_RELEASE_REPO:-fawney19/Aether}"
AETHER_TUNNEL_RELEASE_TAG="${AETHER_TUNNEL_RELEASE_TAG:-}"
DOCKER_BUILD_CACHE="${DOCKER_BUILD_CACHE:-0}"
DOCKER_BUILD_CACHE_DIR="${DOCKER_BUILD_CACHE_DIR:-.docker-cache/app}"
DOCKER_BUILD_CACHE_MODE="${DOCKER_BUILD_CACHE_MODE:-max}"
export LOCAL_APP_IMAGE

detect_build_version() {
    if command -v git >/dev/null 2>&1; then
        local version
        if version=$(git describe --tags --match 'v[0-9]*' --always --dirty 2>/dev/null); then
            if [ -n "$version" ]; then
                printf '%s\n' "$version"
                return 0
            fi
        fi
    fi

    printf 'local-%s\n' "$(date -u +%Y%m%d%H%M%S)"
}

AETHER_BUILD_VERSION="${AETHER_BUILD_VERSION:-$(detect_build_version)}"
export AETHER_BUILD_VERSION

# 缓存文件
CODE_HASH_FILE=".code-hash"

usage() {
    cat <<'EOF'
Usage: ./deploy.sh [options]

Options:
  --force, -f             强制重建镜像
  --tag, -t TAG           额外打自定义 tag（始终保留 latest）
  --tunnel-mode MODE      aether-tunnel 打包方式：source/release/none，默认 source
  --tunnel-release-tag TAG
                          release 模式下载的上游 tag，例如 tunnel-v0.3.13
  --tunnel-release-repo REPO
                          release 模式下载的仓库，默认 fawney19/Aether
  --build-cache           启用 Docker BuildKit 本地缓存导入/导出
  --no-build-cache        禁用 Docker BuildKit 本地缓存导入/导出，默认禁用
  --build-cache-dir DIR   Docker BuildKit 本地缓存目录，默认 .docker-cache/app
  -h, --help              显示帮助

Environment:
  LOCAL_APP_IMAGE          本地构建镜像名，默认 aether-app:latest
  AETHER_BUILD_VERSION     应用显示版本，默认 git describe --tags --match 'v[0-9]*' --always --dirty
  AETHER_TUNNEL_MODE       source 从当前源码构建；release 下载上游二进制；none 不打包
  AETHER_TUNNEL_RELEASE_TAG
                           release 模式必填，例如 tunnel-v0.3.13
  AETHER_TUNNEL_RELEASE_REPO
                           release 模式下载仓库，默认 fawney19/Aether
  DOCKER_BUILD_CACHE       是否启用 Docker BuildKit 本地缓存，默认 0；设为 1 启用
  DOCKER_BUILD_CACHE_DIR   Docker BuildKit 本地缓存目录，默认 .docker-cache/app
  DOCKER_BUILD_CACHE_MODE  缓存导出模式，默认 max；可设为 min/max
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

validate_tunnel_mode() {
    case "$AETHER_TUNNEL_MODE" in
        source|release|none) ;;
        *)
            echo "Invalid AETHER_TUNNEL_MODE: ${AETHER_TUNNEL_MODE}"
            echo "Allowed values: source, release, none"
            exit 1
            ;;
    esac

    if [ "$AETHER_TUNNEL_MODE" = "release" ] && [ -z "$AETHER_TUNNEL_RELEASE_TAG" ]; then
        echo "AETHER_TUNNEL_RELEASE_TAG is required when AETHER_TUNNEL_MODE=release"
        echo "Example: AETHER_TUNNEL_MODE=release AETHER_TUNNEL_RELEASE_TAG=tunnel-v0.3.13 ./deploy.sh"
        exit 1
    fi
}

print_result() {
    echo ">>> Done!"
    echo ">>> Built image: ${LOCAL_APP_IMAGE}"
    echo ">>> Build version: ${AETHER_BUILD_VERSION}"
    echo ">>> Aether tunnel mode: ${AETHER_TUNNEL_MODE}"
    if [ "$AETHER_TUNNEL_MODE" = "release" ]; then
        echo ">>> Aether tunnel release: ${AETHER_TUNNEL_RELEASE_REPO}@${AETHER_TUNNEL_RELEASE_TAG}"
    fi
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
        --tunnel-mode)
            if [ $# -lt 2 ]; then
                echo "Missing value for $1"
                usage
                exit 1
            fi
            AETHER_TUNNEL_MODE="$2"
            shift 2
            ;;
        --tunnel-release-tag)
            if [ $# -lt 2 ]; then
                echo "Missing value for $1"
                usage
                exit 1
            fi
            AETHER_TUNNEL_RELEASE_TAG="$2"
            shift 2
            ;;
        --tunnel-release-repo)
            if [ $# -lt 2 ]; then
                echo "Missing value for $1"
                usage
                exit 1
            fi
            AETHER_TUNNEL_RELEASE_REPO="$2"
            shift 2
            ;;
        --build-cache)
            DOCKER_BUILD_CACHE=1
            shift
            ;;
        --no-build-cache)
            DOCKER_BUILD_CACHE=0
            shift
            ;;
        --build-cache-dir)
            if [ $# -lt 2 ]; then
                echo "Missing value for $1"
                usage
                exit 1
            fi
            DOCKER_BUILD_CACHE_DIR="$2"
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

validate_tunnel_mode

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

emit_value_for_hash() {
    local name="$1"
    local value="$2"
    printf '\n>>> %s\n%s\n' "$name" "$value"
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
        emit_value_for_hash AETHER_BUILD_VERSION "$AETHER_BUILD_VERSION"
        emit_value_for_hash AETHER_TUNNEL_MODE "$AETHER_TUNNEL_MODE"
        emit_value_for_hash AETHER_TUNNEL_RELEASE_REPO "$AETHER_TUNNEL_RELEASE_REPO"
        emit_value_for_hash AETHER_TUNNEL_RELEASE_TAG "$AETHER_TUNNEL_RELEASE_TAG"

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

docker_build_cache_enabled() {
    case "${DOCKER_BUILD_CACHE}" in
        1|true|TRUE|yes|YES|on|ON) return 0 ;;
        0|false|FALSE|no|NO|off|OFF) return 1 ;;
        *)
            echo "Invalid DOCKER_BUILD_CACHE: ${DOCKER_BUILD_CACHE}"
            echo "Allowed values: 1/0, true/false, yes/no, on/off"
            exit 1
            ;;
    esac
}

safe_remove_path() {
    local path="$1"
    if [ -z "$path" ] || [ "$path" = "/" ] || [ "$path" = "." ] || [ "$path" = ".." ]; then
        echo "Refusing to remove unsafe path: ${path:-<empty>}"
        exit 1
    fi
    rm -rf -- "$path"
}

cleanup_disabled_docker_build_cache() {
    local cache_tmp="${DOCKER_BUILD_CACHE_DIR}.tmp"
    if [ -e "$cache_tmp" ]; then
        echo ">>> Removing stale Docker BuildKit cache temp dir: ${cache_tmp}"
        safe_remove_path "$cache_tmp"
    fi

    docker_build_cache_enabled && return 0

    if [ -e "$DOCKER_BUILD_CACHE_DIR" ]; then
        echo ">>> Removing Docker BuildKit cache created by deploy.sh: ${DOCKER_BUILD_CACHE_DIR}"
        safe_remove_path "$DOCKER_BUILD_CACHE_DIR"
    fi
}

validate_docker_build_cache_mode() {
    case "${DOCKER_BUILD_CACHE_MODE}" in
        min|max) ;;
        *)
            echo "Invalid DOCKER_BUILD_CACHE_MODE: ${DOCKER_BUILD_CACHE_MODE}"
            echo "Allowed values: min, max"
            exit 1
            ;;
    esac
}

docker_buildx_driver() {
    docker buildx inspect 2>/dev/null | awk -F: '
        /^Driver:/ {
            gsub(/^[[:space:]]+|[[:space:]]+$/, "", $2)
            print $2
            exit
        }
    '
}

docker_buildx_supports_external_cache() {
    docker buildx version >/dev/null 2>&1 || return 1

    local driver
    driver="$(docker_buildx_driver)"
    [ -n "$driver" ] || return 1

    # docker driver 不支持 --cache-to 外部缓存导出，除非 Docker 开启 containerd image store。
    # 为兼容普通服务器默认配置，这里只在非 docker driver 下启用本地 cache-to/cache-from。
    [ "$driver" != "docker" ]
}

prepare_docker_build_cache_args() {
    local -n out_args=$1
    local -n out_tmp=$2
    out_args=()
    out_tmp=""

    docker_build_cache_enabled || return 0
    validate_docker_build_cache_mode

    if ! docker_buildx_supports_external_cache; then
        echo ">>> Docker buildx external cache is not available for the current builder; using Docker's local layer/cache-mount cache."
        if docker image inspect "$LOCAL_APP_IMAGE" >/dev/null 2>&1; then
            out_args+=(--cache-from "$LOCAL_APP_IMAGE")
        fi
        return 0
    fi

    mkdir -p "$(dirname "$DOCKER_BUILD_CACHE_DIR")"
    out_tmp="${DOCKER_BUILD_CACHE_DIR}.tmp"
    rm -rf "$out_tmp"

    if [ -d "$DOCKER_BUILD_CACHE_DIR" ]; then
        out_args+=(--cache-from "type=local,src=${DOCKER_BUILD_CACHE_DIR}")
    fi
    if docker image inspect "$LOCAL_APP_IMAGE" >/dev/null 2>&1; then
        out_args+=(--cache-from "$LOCAL_APP_IMAGE")
    fi
    out_args+=(--cache-to "type=local,dest=${out_tmp},mode=${DOCKER_BUILD_CACHE_MODE}")
    echo ">>> Docker BuildKit cache: ${DOCKER_BUILD_CACHE_DIR} (mode=${DOCKER_BUILD_CACHE_MODE})"
}

docker_build_command() {
    if docker_build_cache_enabled && docker_buildx_supports_external_cache; then
        printf '%s\n' "docker" "buildx" "build" "--load"
    else
        printf '%s\n' "docker" "build"
    fi
}

promote_docker_build_cache() {
    local cache_tmp="$1"
    [ -n "$cache_tmp" ] || return 0
    [ -d "$cache_tmp" ] || return 0
    rm -rf "$DOCKER_BUILD_CACHE_DIR"
    mv "$cache_tmp" "$DOCKER_BUILD_CACHE_DIR"
}

# 构建应用镜像
build_app() {
    require_file Dockerfile.app.local
    echo ">>> Building app image: $LOCAL_APP_IMAGE"
    echo ">>> Build version: $AETHER_BUILD_VERSION"
    local build_args=(
        --build-arg "BUILDKIT_INLINE_CACHE=1"
        --build-arg "AETHER_BUILD_VERSION=${AETHER_BUILD_VERSION}"
        --build-arg "AETHER_TUNNEL_MODE=${AETHER_TUNNEL_MODE}"
        --build-arg "AETHER_TUNNEL_RELEASE_REPO=${AETHER_TUNNEL_RELEASE_REPO}"
        --build-arg "AETHER_TUNNEL_RELEASE_TAG=${AETHER_TUNNEL_RELEASE_TAG}"
    )
    local cache_args=()
    local cache_tmp=""
    local docker_cmd=()
    prepare_docker_build_cache_args cache_args cache_tmp
    mapfile -t docker_cmd < <(docker_build_command)

    if ! DOCKER_BUILDKIT="${DOCKER_BUILDKIT:-1}" "${docker_cmd[@]}" --pull=false "${cache_args[@]}" "${build_args[@]}" -f Dockerfile.app.local -t "$LOCAL_APP_IMAGE" .; then
        [ -n "$cache_tmp" ] && rm -rf "$cache_tmp"
        return 1
    fi
    promote_docker_build_cache "$cache_tmp"
    apply_custom_tag
    save_code_hash
}

cleanup_disabled_docker_build_cache

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
