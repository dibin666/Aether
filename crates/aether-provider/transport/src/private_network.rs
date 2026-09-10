//! Administrator-granted private upstream allowances.
//!
//! Execution refuses private and reserved upstream addresses by default. A
//! provider endpoint that genuinely lives on an internal network opts out of
//! that refusal one endpoint at a time, through an explicit `config` section on
//! the saved endpoint row. The allowance derived here is a single origin — the
//! scheme, literal address, and port of the endpoint's own saved `base_url` —
//! so a granted endpoint can only ever reach the target the administrator
//! already wrote down.
//!
//! Two properties are load bearing and are the reason this module exists
//! instead of a global "allow private targets" switch:
//!
//! * The allowance is derived from stored provider catalog rows, never from an
//!   execution plan or a client request. Request-side input can only be matched
//!   against an allowance, never create one.
//! * Only literal-IP base URLs are eligible. A hostname would have to be
//!   resolved, and a resolver answer can drift between the check and the
//!   connect, so hostnames stay on the default-deny path.

use std::net::IpAddr;

use aether_http::is_private_or_reserved_ip;
use serde_json::Value;
use url::{Host, Url};

/// Provider endpoint `config` key holding the private-network allowance.
pub const ENDPOINT_PRIVATE_NETWORK_ACCESS_CONFIG_KEY: &str = "private_network_access";

/// A single upstream origin an administrator has explicitly opened up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrivateUpstreamOrigin {
    scheme: &'static str,
    address: IpAddr,
    port: u16,
}

impl PrivateUpstreamOrigin {
    pub fn scheme(&self) -> &'static str {
        self.scheme
    }

    pub fn address(&self) -> IpAddr {
        self.address
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Render the allowance for logs and admin responses.
    ///
    /// Only the origin is exposed. Endpoint paths and query strings routinely
    /// carry API keys and signed parameters, so they never reach a log line.
    pub fn origin(&self) -> String {
        format!(
            "{}://{}",
            self.scheme,
            format_authority(self.address, self.port)
        )
    }

    /// Whether `url` targets exactly this allowed origin.
    ///
    /// The address is compared as a parsed `IpAddr`, so alternate spellings of
    /// the same target (`0177.0.0.1`, `::ffff:10.0.0.1`) either normalize to
    /// the allowed address or fail to match. They can never widen it.
    pub fn allows(&self, url: &Url) -> bool {
        url.scheme() == self.scheme
            && url.port_or_known_default() == Some(self.port)
            && url_literal_ip_host(url) == Some(self.address)
    }
}

/// Read the private-network allowance off a saved provider endpoint.
///
/// `Ok(None)` means the endpoint did not ask for one. `Err` carries an
/// administrator-facing reason and is used to reject the write; the execution
/// path treats an `Err` exactly like `Ok(None)` and keeps denying the target.
pub fn resolve_endpoint_private_upstream_origin(
    base_url: &str,
    config: Option<&Value>,
) -> Result<Option<PrivateUpstreamOrigin>, String> {
    let Some(section) = config
        .and_then(|config| config.get(ENDPOINT_PRIVATE_NETWORK_ACCESS_CONFIG_KEY))
        .filter(|section| !section.is_null())
    else {
        return Ok(None);
    };
    let Some(section) = section.as_object() else {
        return Err(format!(
            "config.{ENDPOINT_PRIVATE_NETWORK_ACCESS_CONFIG_KEY} 必须是对象或 null"
        ));
    };
    let Some(enabled) = section.get("enabled").and_then(Value::as_bool) else {
        return Err(format!(
            "config.{ENDPOINT_PRIVATE_NETWORK_ACCESS_CONFIG_KEY}.enabled 必须是布尔值"
        ));
    };
    if !enabled {
        return Ok(None);
    }

    let origin = private_upstream_origin_from_base_url(base_url)?;

    // An optional pin. When present the administrator has written the target
    // down twice, so an unrelated `base_url` edit can no longer silently move
    // the allowance to a different internal host.
    if let Some(target) = section.get("target").filter(|target| !target.is_null()) {
        let Some(target) = target.as_str() else {
            return Err(format!(
                "config.{ENDPOINT_PRIVATE_NETWORK_ACCESS_CONFIG_KEY}.target 必须是字符串或 null"
            ));
        };
        let pinned = parse_pinned_target(origin.scheme, target)?;
        if pinned != origin {
            return Err(format!(
                "config.{ENDPOINT_PRIVATE_NETWORK_ACCESS_CONFIG_KEY}.target 与 base_url 的地址和端口不一致"
            ));
        }
    }

    Ok(Some(origin))
}

fn private_upstream_origin_from_base_url(base_url: &str) -> Result<PrivateUpstreamOrigin, String> {
    let url = Url::parse(base_url.trim()).map_err(|_| {
        format!("启用 {ENDPOINT_PRIVATE_NETWORK_ACCESS_CONFIG_KEY} 需要 base_url 是合法的 URL")
    })?;
    let scheme = match url.scheme() {
        "http" => "http",
        "https" => "https",
        _ => {
            return Err(format!(
                "启用 {ENDPOINT_PRIVATE_NETWORK_ACCESS_CONFIG_KEY} 需要 base_url 使用 http 或 https"
            ))
        }
    };
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(format!(
            "启用 {ENDPOINT_PRIVATE_NETWORK_ACCESS_CONFIG_KEY} 的 base_url 不能包含凭证或 fragment"
        ));
    }
    let Some(address) = url_literal_ip_host(&url) else {
        return Err(format!(
            "{ENDPOINT_PRIVATE_NETWORK_ACCESS_CONFIG_KEY} 只支持字面 IP 的 base_url；主机名的解析结果可能漂移"
        ));
    };
    if !is_private_or_reserved_ip(address) {
        return Err(format!(
            "base_url 不是私网或保留地址，无需启用 {ENDPOINT_PRIVATE_NETWORK_ACCESS_CONFIG_KEY}"
        ));
    }
    let Some(port) = url.port_or_known_default() else {
        return Err(format!(
            "启用 {ENDPOINT_PRIVATE_NETWORK_ACCESS_CONFIG_KEY} 需要 base_url 能确定端口"
        ));
    };
    Ok(PrivateUpstreamOrigin {
        scheme,
        address,
        port,
    })
}

fn parse_pinned_target(
    scheme: &'static str,
    target: &str,
) -> Result<PrivateUpstreamOrigin, String> {
    let target = target.trim();
    let parsed = Url::parse(&format!("{scheme}://{target}")).ok();
    let parsed = parsed
        .filter(|url| url.path() == "/" && url.query().is_none() && url.fragment().is_none())
        .filter(|url| url.username().is_empty() && url.password().is_none());
    let address = parsed.as_ref().and_then(url_literal_ip_host);
    let port = parsed.as_ref().and_then(Url::port_or_known_default);
    match (address, port) {
        (Some(address), Some(port)) => Ok(PrivateUpstreamOrigin {
            scheme,
            address,
            port,
        }),
        _ => Err(format!(
            "config.{ENDPOINT_PRIVATE_NETWORK_ACCESS_CONFIG_KEY}.target 必须是 IP 或 IP:端口"
        )),
    }
}

fn url_literal_ip_host(url: &Url) -> Option<IpAddr> {
    match url.host()? {
        Host::Ipv4(address) => Some(IpAddr::V4(address)),
        Host::Ipv6(address) => Some(IpAddr::V6(address)),
        Host::Domain(_) => None,
    }
}

fn format_authority(address: IpAddr, port: u16) -> String {
    match address {
        IpAddr::V4(address) => format!("{address}:{port}"),
        IpAddr::V6(address) => format!("[{address}]:{port}"),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn enabled_config() -> Value {
        json!({"private_network_access": {"enabled": true}})
    }

    #[test]
    fn absent_or_disabled_sections_grant_nothing() {
        for config in [
            None,
            Some(json!({})),
            Some(json!({"private_network_access": null})),
            Some(json!({"private_network_access": {"enabled": false}})),
        ] {
            assert_eq!(
                resolve_endpoint_private_upstream_origin(
                    "http://10.0.0.106:8317/v1",
                    config.as_ref()
                ),
                Ok(None)
            );
        }
    }

    #[test]
    fn enabled_literal_private_base_url_grants_exactly_one_origin() {
        let origin = resolve_endpoint_private_upstream_origin(
            "http://10.0.0.106:8317/v1",
            Some(&enabled_config()),
        )
        .expect("an explicit private endpoint should resolve")
        .expect("an enabled section should grant an allowance");

        assert_eq!(origin.origin(), "http://10.0.0.106:8317");
        assert!(origin.allows(&Url::parse("http://10.0.0.106:8317/v1/messages").unwrap()));
        assert!(origin.allows(&Url::parse("http://10.0.0.106:8317/v1/models?x=1").unwrap()));
    }

    #[test]
    fn granted_origins_reject_neighbouring_hosts_ports_and_schemes() {
        let origin = resolve_endpoint_private_upstream_origin(
            "http://10.0.0.106:8317/v1",
            Some(&enabled_config()),
        )
        .unwrap()
        .unwrap();

        for rejected in [
            "http://10.0.0.107:8317/v1/messages",
            "http://10.0.0.106:8318/v1/messages",
            "http://10.0.0.106/v1/messages",
            "https://10.0.0.106:8317/v1/messages",
            "http://169.254.169.254:8317/latest/meta-data",
            "http://[::ffff:10.0.0.106]:8317/v1/messages",
            "http://internal.example.test:8317/v1/messages",
        ] {
            assert!(
                !origin.allows(&Url::parse(rejected).unwrap()),
                "allowance must not cover {rejected}"
            );
        }
    }

    #[test]
    fn hostname_and_public_base_urls_cannot_be_granted() {
        for base_url in [
            "http://internal.corp.test:8317/v1",
            "https://api.example.test/v1",
            "http://8.8.8.8:8317/v1",
            "ftp://10.0.0.106:8317",
            "http://user:pass@10.0.0.106:8317/v1",
            "http://10.0.0.106:8317/v1#fragment",
            "not-a-url",
        ] {
            assert!(
                resolve_endpoint_private_upstream_origin(base_url, Some(&enabled_config())).is_err(),
                "base_url should be refused: {base_url}"
            );
        }
    }

    #[test]
    fn malformed_sections_are_rejected_rather_than_ignored() {
        for config in [
            json!({"private_network_access": true}),
            json!({"private_network_access": {}}),
            json!({"private_network_access": {"enabled": "true"}}),
            json!({"private_network_access": {"enabled": 1}}),
        ] {
            assert!(
                resolve_endpoint_private_upstream_origin("http://10.0.0.106:8317/v1", Some(&config))
                    .is_err(),
                "malformed section should be refused: {config}"
            );
        }
    }

    #[test]
    fn pinned_targets_must_agree_with_the_base_url() {
        let matching = json!({
            "private_network_access": {"enabled": true, "target": "10.0.0.106:8317"},
        });
        assert_eq!(
            resolve_endpoint_private_upstream_origin(
                "http://10.0.0.106:8317/v1",
                Some(&matching)
            )
            .unwrap()
            .unwrap()
            .origin(),
            "http://10.0.0.106:8317"
        );

        for target in ["10.0.0.107:8317", "10.0.0.106:8318", "10.0.0.106", "", "10"] {
            let config = json!({
                "private_network_access": {"enabled": true, "target": target},
            });
            assert!(
                resolve_endpoint_private_upstream_origin(
                    "http://10.0.0.106:8317/v1",
                    Some(&config)
                )
                .is_err(),
                "pinned target should be refused: {target}"
            );
        }
    }

    #[test]
    fn ipv6_private_base_urls_round_trip_through_the_pin() {
        let config = json!({
            "private_network_access": {"enabled": true, "target": "[fd00::1]:8443"},
        });
        let origin =
            resolve_endpoint_private_upstream_origin("http://[fd00::1]:8443/v1", Some(&config))
                .unwrap()
                .unwrap();

        assert_eq!(origin.origin(), "http://[fd00::1]:8443");
        assert!(origin.allows(&Url::parse("http://[fd00::1]:8443/v1/messages").unwrap()));
        assert!(!origin.allows(&Url::parse("http://[fd00::2]:8443/v1/messages").unwrap()));
    }
}
