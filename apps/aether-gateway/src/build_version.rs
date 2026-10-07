//! 网关对外报告的版本号。
//!
//! CI 在仅前端变更时会复用上一次编译的二进制，其编译期版本号会落后于镜像版本；
//! 镜像通过 `AETHER_IMAGE_VERSION` 注入当前版本，运行时优先使用它。

use std::sync::OnceLock;

const IMAGE_VERSION_ENV: &str = "AETHER_IMAGE_VERSION";

pub(crate) fn current_build_version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION
        .get_or_init(|| resolve_build_version(std::env::var(IMAGE_VERSION_ENV).ok().as_deref()))
        .as_str()
}

fn resolve_build_version(image_version: Option<&str>) -> String {
    image_version
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(|version| version.strip_prefix('v').unwrap_or(version).to_string())
        .unwrap_or_else(|| {
            option_env!("AETHER_BUILD_VERSION")
                .filter(|version| !version.is_empty())
                .unwrap_or(env!("CARGO_PKG_VERSION"))
                .to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::resolve_build_version;

    #[test]
    fn image_version_overrides_compiled_version() {
        assert_eq!(
            resolve_build_version(Some(" aether-m14.6 ")),
            "aether-m14.6"
        );
        assert_eq!(resolve_build_version(Some("v1.2.3")), "1.2.3");
    }

    #[test]
    fn blank_image_version_falls_back_to_compiled_version() {
        let compiled = option_env!("AETHER_BUILD_VERSION")
            .filter(|version| !version.is_empty())
            .unwrap_or(env!("CARGO_PKG_VERSION"));
        assert_eq!(resolve_build_version(None), compiled);
        assert_eq!(resolve_build_version(Some("  ")), compiled);
    }
}
