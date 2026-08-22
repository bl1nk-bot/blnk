//! Security policy boundary shared by future TCP, WebSocket, and HTTP proxy streams.
//!
//! This module deliberately does not open sockets or implement a stream handler. A
//! handler must validate the target before connecting, resolve host names through
//! [`ProxyPolicy::resolve_and_validate`], pin the returned addresses for retries,
//! and validate every redirect again.

use std::{
    collections::BTreeSet,
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use thiserror::Error;
use url::Url;

use crate::utils::error::BlnkError;

pub const REDACTED_VALUE: &str = "<redacted>";

/// Schemes that can be handled by the proxy stream family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProxyScheme {
    Http,
    Https,
    Tcp,
    Ws,
    Wss,
}

impl ProxyScheme {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
            Self::Tcp => "tcp",
            Self::Ws => "ws",
            Self::Wss => "wss",
        }
    }

    const fn default_port(self) -> Option<u16> {
        match self {
            Self::Http | Self::Ws => Some(80),
            Self::Https | Self::Wss => Some(443),
            Self::Tcp => None,
        }
    }

    const fn is_secure(self) -> bool {
        matches!(self, Self::Https | Self::Wss)
    }

    fn parse(scheme: &str) -> ProxyResult<Self> {
        match scheme {
            "http" => Ok(Self::Http),
            "https" => Ok(Self::Https),
            "tcp" => Ok(Self::Tcp),
            "ws" => Ok(Self::Ws),
            "wss" => Ok(Self::Wss),
            _ => Err(ProxyPolicyError::UnsupportedScheme),
        }
    }
}

impl fmt::Display for ProxyScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The proof a caller has obtained before asking the policy to authorize a target.
///
/// `Allowlisted` is suitable for a configured target. `UserConfirmed` is a
/// per-request confirmation and is disabled by the default policy. `None` is
/// intentionally rejected even for public addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyAuthorization {
    None,
    Allowlisted,
    UserConfirmed,
}

/// Stable, non-sensitive reasons returned by policy validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProxyPolicyError {
    #[error("authorization_required")]
    AuthorizationRequired,
    #[error("target_not_allowlisted")]
    TargetNotAllowlisted,
    #[error("user_confirmation_required")]
    UserConfirmationRequired,
    #[error("unsupported_scheme")]
    UnsupportedScheme,
    #[error("host_required")]
    HostRequired,
    #[error("credentials_not_allowed")]
    CredentialsNotAllowed,
    #[error("fragment_not_allowed")]
    FragmentNotAllowed,
    #[error("invalid_target")]
    InvalidTarget,
    #[error("invalid_port")]
    InvalidPort,
    #[error("port_not_allowed")]
    PortNotAllowed,
    #[error("address_not_allowed")]
    AddressNotAllowed,
    #[error("no_resolved_addresses")]
    NoResolvedAddresses,
    #[error("resolution_failed")]
    ResolutionFailed,
    #[error("resolved_port_mismatch")]
    ResolvedPortMismatch,
    #[error("redirect_limit_exceeded")]
    RedirectLimitExceeded,
    #[error("cross_origin_redirect")]
    CrossOriginRedirect,
    #[error("insecure_redirect")]
    InsecureRedirect,
    #[error("dns_rebinding_detected")]
    DnsRebindingDetected,
    #[error("request_limit_exceeded")]
    RequestLimitExceeded,
    #[error("response_limit_exceeded")]
    ResponseLimitExceeded,
    #[error("concurrency_limit_exceeded")]
    ConcurrencyLimitExceeded,
    #[error("backpressure_limit_exceeded")]
    BackpressureLimitExceeded,
    #[error("invalid_limits")]
    InvalidLimits,
}

impl ProxyPolicyError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::AuthorizationRequired => "authorization_required",
            Self::TargetNotAllowlisted => "target_not_allowlisted",
            Self::UserConfirmationRequired => "user_confirmation_required",
            Self::UnsupportedScheme => "unsupported_scheme",
            Self::HostRequired => "host_required",
            Self::CredentialsNotAllowed => "credentials_not_allowed",
            Self::FragmentNotAllowed => "fragment_not_allowed",
            Self::InvalidTarget => "invalid_target",
            Self::InvalidPort => "invalid_port",
            Self::PortNotAllowed => "port_not_allowed",
            Self::AddressNotAllowed => "address_not_allowed",
            Self::NoResolvedAddresses => "no_resolved_addresses",
            Self::ResolutionFailed => "resolution_failed",
            Self::ResolvedPortMismatch => "resolved_port_mismatch",
            Self::RedirectLimitExceeded => "redirect_limit_exceeded",
            Self::CrossOriginRedirect => "cross_origin_redirect",
            Self::InsecureRedirect => "insecure_redirect",
            Self::DnsRebindingDetected => "dns_rebinding_detected",
            Self::RequestLimitExceeded => "request_limit_exceeded",
            Self::ResponseLimitExceeded => "response_limit_exceeded",
            Self::ConcurrencyLimitExceeded => "concurrency_limit_exceeded",
            Self::BackpressureLimitExceeded => "backpressure_limit_exceeded",
            Self::InvalidLimits => "invalid_limits",
        }
    }
}

impl From<ProxyPolicyError> for BlnkError {
    fn from(error: ProxyPolicyError) -> Self {
        BlnkError::Stream(format!("proxy policy rejected: {}", error.code()))
    }
}

pub type ProxyResult<T> = Result<T, ProxyPolicyError>;

/// Resource ceilings applied by future proxy handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProxyResourceLimits {
    pub max_request_bytes: u64,
    pub max_response_bytes: u64,
    pub max_concurrent_streams: usize,
    pub max_buffered_bytes: usize,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_redirects: u8,
}

impl Default for ProxyResourceLimits {
    fn default() -> Self {
        Self {
            max_request_bytes: 1 << 20,
            max_response_bytes: 8 << 20,
            max_concurrent_streams: 16,
            max_buffered_bytes: 256 << 10,
            connect_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(60),
            max_redirects: 5,
        }
    }
}

impl ProxyResourceLimits {
    pub fn validate(self) -> ProxyResult<()> {
        const MAX_REQUEST_BYTES: u64 = 64 << 20;
        const MAX_RESPONSE_BYTES: u64 = 64 << 20;
        const MAX_CONCURRENT_STREAMS: usize = 1024;
        const MAX_BUFFERED_BYTES: usize = 16 << 20;
        const MAX_CONNECT_TIMEOUT: Duration = Duration::from_secs(300);
        const MAX_IDLE_TIMEOUT: Duration = Duration::from_secs(3600);
        const MAX_REDIRECTS: u8 = 20;

        if self.max_request_bytes == 0
            || self.max_request_bytes > MAX_REQUEST_BYTES
            || self.max_response_bytes == 0
            || self.max_response_bytes > MAX_RESPONSE_BYTES
            || self.max_concurrent_streams == 0
            || self.max_concurrent_streams > MAX_CONCURRENT_STREAMS
            || self.max_buffered_bytes == 0
            || self.max_buffered_bytes > MAX_BUFFERED_BYTES
            || self.connect_timeout.is_zero()
            || self.connect_timeout > MAX_CONNECT_TIMEOUT
            || self.idle_timeout.is_zero()
            || self.idle_timeout > MAX_IDLE_TIMEOUT
            || self.max_redirects > MAX_REDIRECTS
        {
            return Err(ProxyPolicyError::InvalidLimits);
        }
        Ok(())
    }

    pub fn check_request_size(self, bytes: u64) -> ProxyResult<()> {
        if bytes > self.max_request_bytes {
            Err(ProxyPolicyError::RequestLimitExceeded)
        } else {
            Ok(())
        }
    }

    pub fn check_response_size(self, bytes: u64) -> ProxyResult<()> {
        if bytes > self.max_response_bytes {
            Err(ProxyPolicyError::ResponseLimitExceeded)
        } else {
            Ok(())
        }
    }

    pub fn check_buffered_bytes(self, bytes: usize) -> ProxyResult<()> {
        if bytes > self.max_buffered_bytes {
            Err(ProxyPolicyError::BackpressureLimitExceeded)
        } else {
            Ok(())
        }
    }
}

/// Normalized scheme/host/port identity used for exact allowlist and origin checks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProxyTargetKey {
    scheme: ProxyScheme,
    host: String,
    port: u16,
}

impl ProxyTargetKey {
    pub fn scheme(&self) -> ProxyScheme {
        self.scheme
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

/// A URL that passed syntax, scheme, port, authorization, and literal-IP checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProxyTarget {
    url: Url,
    key: ProxyTargetKey,
}

impl ValidatedProxyTarget {
    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn key(&self) -> &ProxyTargetKey {
        &self.key
    }

    pub fn scheme(&self) -> ProxyScheme {
        self.key.scheme
    }

    pub fn host(&self) -> &str {
        &self.key.host
    }

    pub fn port(&self) -> u16 {
        self.key.port
    }
}

/// A validated target plus the DNS answer set pinned for the connection lifetime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProxyTarget {
    target: ValidatedProxyTarget,
    addresses: Vec<SocketAddr>,
}

impl ResolvedProxyTarget {
    pub fn target(&self) -> &ValidatedProxyTarget {
        &self.target
    }

    pub fn addresses(&self) -> &[SocketAddr] {
        &self.addresses
    }
}

/// Deny-by-default policy shared by TCP, WebSocket, and HTTP proxy handlers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyPolicy {
    allowed_schemes: BTreeSet<ProxyScheme>,
    allowed_targets: BTreeSet<ProxyTargetKey>,
    allowed_ports: Option<BTreeSet<u16>>,
    allow_user_confirmation: bool,
    allow_local_targets: bool,
    allow_cross_origin_redirects: bool,
    allow_insecure_redirects: bool,
    limits: ProxyResourceLimits,
}

impl Default for ProxyPolicy {
    fn default() -> Self {
        Self::deny_by_default()
    }
}

impl ProxyPolicy {
    pub fn deny_by_default() -> Self {
        Self {
            allowed_schemes: BTreeSet::from([
                ProxyScheme::Http,
                ProxyScheme::Https,
                ProxyScheme::Tcp,
                ProxyScheme::Ws,
                ProxyScheme::Wss,
            ]),
            allowed_targets: BTreeSet::new(),
            allowed_ports: None,
            allow_user_confirmation: false,
            allow_local_targets: false,
            allow_cross_origin_redirects: false,
            allow_insecure_redirects: false,
            limits: ProxyResourceLimits::default(),
        }
    }

    pub fn with_allowed_target(mut self, target: &Url) -> ProxyResult<Self> {
        self.allowed_targets.insert(Self::target_key(target)?);
        Ok(self)
    }

    pub fn with_allowed_ports<I>(mut self, ports: I) -> ProxyResult<Self>
    where
        I: IntoIterator<Item = u16>,
    {
        let ports: BTreeSet<u16> = ports.into_iter().collect();
        if ports.contains(&0) {
            return Err(ProxyPolicyError::InvalidPort);
        }
        self.allowed_ports = Some(ports);
        Ok(self)
    }

    pub fn with_user_confirmation(mut self, enabled: bool) -> Self {
        self.allow_user_confirmation = enabled;
        self
    }

    /// Opt in to private, loopback, link-local, and IPv6 ULA targets.
    ///
    /// Reserved, unspecified, and multicast addresses remain blocked even after
    /// this opt-in. The opt-in must still be paired with allowlist authorization or
    /// per-request user confirmation.
    pub fn with_local_targets(mut self, enabled: bool) -> Self {
        self.allow_local_targets = enabled;
        self
    }

    pub fn with_redirect_policy(mut self, allow_cross_origin: bool, allow_insecure: bool) -> Self {
        self.allow_cross_origin_redirects = allow_cross_origin;
        self.allow_insecure_redirects = allow_insecure;
        self
    }

    pub fn with_limits(mut self, limits: ProxyResourceLimits) -> ProxyResult<Self> {
        limits.validate()?;
        self.limits = limits;
        Ok(self)
    }

    pub fn limits(&self) -> ProxyResourceLimits {
        self.limits
    }

    pub fn allowed_targets(&self) -> &BTreeSet<ProxyTargetKey> {
        &self.allowed_targets
    }

    /// Validate the target before any DNS lookup or socket connect.
    pub fn validate_target(
        &self,
        target: &Url,
        authorization: ProxyAuthorization,
    ) -> ProxyResult<ValidatedProxyTarget> {
        let key = Self::target_key(target)?;
        if !self.allowed_schemes.contains(&key.scheme) {
            return Err(ProxyPolicyError::UnsupportedScheme);
        }
        if let Some(ports) = &self.allowed_ports
            && !ports.contains(&key.port)
        {
            return Err(ProxyPolicyError::PortNotAllowed);
        }

        match authorization {
            ProxyAuthorization::None => return Err(ProxyPolicyError::AuthorizationRequired),
            ProxyAuthorization::Allowlisted if !self.allowed_targets.contains(&key) => {
                return Err(ProxyPolicyError::TargetNotAllowlisted);
            }
            ProxyAuthorization::UserConfirmed if !self.allow_user_confirmation => {
                return Err(ProxyPolicyError::UserConfirmationRequired);
            }
            ProxyAuthorization::Allowlisted | ProxyAuthorization::UserConfirmed => {}
        }

        if let Ok(ip) = key.host.parse::<IpAddr>() {
            self.validate_ip(ip)?;
        }

        Ok(ValidatedProxyTarget {
            url: target.clone(),
            key,
        })
    }

    /// Validate all answers from one DNS resolution and pin them for retries.
    ///
    /// A mixed DNS answer set is rejected when even one answer is disallowed. This
    /// prevents a later connection attempt from selecting a private answer from the
    /// same hostname.
    pub fn validate_resolved_target(
        &self,
        target: &Url,
        authorization: ProxyAuthorization,
        addresses: &[SocketAddr],
    ) -> ProxyResult<ResolvedProxyTarget> {
        let validated = self.validate_target(target, authorization)?;
        self.validate_resolved_target_for(validated, addresses)
    }

    /// Resolve a hostname once, validate every answer, and return the pinned set.
    pub async fn resolve_and_validate(
        &self,
        target: &Url,
        authorization: ProxyAuthorization,
    ) -> ProxyResult<ResolvedProxyTarget> {
        let validated = self.validate_target(target, authorization)?;
        let resolved = tokio::net::lookup_host((validated.host(), validated.port()))
            .await
            .map_err(|_| ProxyPolicyError::ResolutionFailed)?;
        let addresses: Vec<SocketAddr> = resolved.collect();
        self.validate_resolved_target_for(validated, &addresses)
    }

    /// Validate a redirect before following it. Relative URL joining belongs to
    /// the HTTP handler; the resulting absolute URL must come through this method.
    pub fn validate_redirect(
        &self,
        original: &ValidatedProxyTarget,
        next: &Url,
        authorization: ProxyAuthorization,
        redirect_count: u8,
    ) -> ProxyResult<ValidatedProxyTarget> {
        if redirect_count >= self.limits.max_redirects {
            return Err(ProxyPolicyError::RedirectLimitExceeded);
        }
        let next = self.validate_target(next, authorization)?;
        if original.scheme().is_secure()
            && !next.scheme().is_secure()
            && !self.allow_insecure_redirects
        {
            return Err(ProxyPolicyError::InsecureRedirect);
        }
        if !self.allow_cross_origin_redirects && original.key() != next.key() {
            return Err(ProxyPolicyError::CrossOriginRedirect);
        }
        Ok(next)
    }

    /// Allow retries only to an address from the original validated DNS answer set.
    pub fn validate_retry(
        &self,
        resolved: &ResolvedProxyTarget,
        retry_address: SocketAddr,
    ) -> ProxyResult<()> {
        self.validate_ip(retry_address.ip())?;
        if retry_address.port() != resolved.target.port() {
            return Err(ProxyPolicyError::ResolvedPortMismatch);
        }
        if !resolved.addresses.contains(&retry_address) {
            return Err(ProxyPolicyError::DnsRebindingDetected);
        }
        Ok(())
    }

    pub fn check_request_size(&self, bytes: u64) -> ProxyResult<()> {
        if bytes > self.limits.max_request_bytes {
            Err(ProxyPolicyError::RequestLimitExceeded)
        } else {
            Ok(())
        }
    }

    pub fn check_response_size(&self, bytes: u64) -> ProxyResult<()> {
        if bytes > self.limits.max_response_bytes {
            Err(ProxyPolicyError::ResponseLimitExceeded)
        } else {
            Ok(())
        }
    }

    pub fn check_concurrent_streams(&self, active_streams: usize) -> ProxyResult<()> {
        if active_streams >= self.limits.max_concurrent_streams {
            Err(ProxyPolicyError::ConcurrencyLimitExceeded)
        } else {
            Ok(())
        }
    }

    pub fn check_buffered_bytes(&self, bytes: usize) -> ProxyResult<()> {
        if bytes > self.limits.max_buffered_bytes {
            Err(ProxyPolicyError::BackpressureLimitExceeded)
        } else {
            Ok(())
        }
    }

    fn target_key(target: &Url) -> ProxyResult<ProxyTargetKey> {
        let scheme = ProxyScheme::parse(target.scheme())?;
        if !target.username().is_empty() || target.password().is_some() {
            return Err(ProxyPolicyError::CredentialsNotAllowed);
        }
        if target.fragment().is_some() {
            return Err(ProxyPolicyError::FragmentNotAllowed);
        }
        let host = target
            .host_str()
            .ok_or(ProxyPolicyError::HostRequired)?
            .trim_end_matches('.')
            .to_ascii_lowercase();
        if host.is_empty() {
            return Err(ProxyPolicyError::HostRequired);
        }
        let explicit_port = target.port();
        if explicit_port == Some(0) {
            return Err(ProxyPolicyError::InvalidPort);
        }
        let port = match explicit_port.or_else(|| scheme.default_port()) {
            Some(port) if port != 0 => port,
            _ => return Err(ProxyPolicyError::InvalidPort),
        };
        if scheme == ProxyScheme::Tcp
            && ((!target.path().is_empty() && target.path() != "/") || target.query().is_some())
        {
            return Err(ProxyPolicyError::InvalidTarget);
        }
        Ok(ProxyTargetKey { scheme, host, port })
    }

    fn validate_resolved_target_for(
        &self,
        target: ValidatedProxyTarget,
        addresses: &[SocketAddr],
    ) -> ProxyResult<ResolvedProxyTarget> {
        if addresses.is_empty() {
            return Err(ProxyPolicyError::NoResolvedAddresses);
        }
        for address in addresses {
            if address.port() != target.port() {
                return Err(ProxyPolicyError::ResolvedPortMismatch);
            }
            self.validate_ip(address.ip())?;
        }
        Ok(ResolvedProxyTarget {
            target,
            addresses: addresses.to_vec(),
        })
    }

    fn validate_ip(&self, ip: IpAddr) -> ProxyResult<()> {
        if is_always_disallowed(ip) || (is_private_or_local(ip) && !self.allow_local_targets) {
            return Err(ProxyPolicyError::AddressNotAllowed);
        }
        Ok(())
    }
}

fn is_always_disallowed(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(address) => {
            address.is_unspecified()
                || address.is_multicast()
                || address.is_broadcast()
                || is_reserved_ipv4(address)
        }
        IpAddr::V6(address) => {
            address.is_unspecified()
                || address.is_multicast()
                || is_reserved_ipv6(address)
                || address
                    .to_ipv4_mapped()
                    .is_some_and(|mapped| is_always_disallowed(IpAddr::V4(mapped)))
        }
    }
}

fn is_private_or_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(address) => {
            address.is_private() || address.is_loopback() || address.is_link_local()
        }
        IpAddr::V6(address) => {
            address.is_loopback()
                || is_ipv6_unique_local_or_link_local(address)
                || address
                    .to_ipv4_mapped()
                    .is_some_and(|mapped| is_private_or_local(IpAddr::V4(mapped)))
        }
    }
}

fn is_reserved_ipv4(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    matches!(
        octets,
        [0, _, _, _]
            | [100, 64..=127, _, _]
            | [192, 0, 0, _]
            | [192, 0, 2, _]
            | [192, 88, 99, _]
            | [198, 18..=19, _, _]
            | [198, 51, 100, _]
            | [203, 0, 113, _]
            | [240..=255, _, _, _]
    )
}

fn is_reserved_ipv6(address: Ipv6Addr) -> bool {
    let segments = address.segments();
    segments[0] == 0x2001 && segments[1] == 0x0db8
}

fn is_ipv6_unique_local_or_link_local(address: Ipv6Addr) -> bool {
    let segments = address.segments();
    (segments[0] & 0xfe00) == 0xfc00 || (segments[0] & 0xffc0) == 0xfe80
}

/// Return a log-safe target label. Path, query, fragment, and credentials are
/// intentionally omitted; callers should use this instead of logging a raw URL.
pub fn redact_target_for_log(target: &Url) -> String {
    let Some(host) = target.host_str() else {
        return "<invalid-target>".to_owned();
    };
    let port = target
        .port_or_known_default()
        .map_or_else(|| "?".to_owned(), |port| port.to_string());
    let authority = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    format!("{}://{authority}", target.scheme())
}

pub fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "authorization"
            | "proxy-authorization"
            | "cookie"
            | "set-cookie"
            | "www-authenticate"
            | "x-api-key"
            | "x-auth-token"
    )
}

pub fn redact_header_value(name: &str, value: &str) -> String {
    if is_sensitive_header(name) {
        REDACTED_VALUE.to_owned()
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn public_target() -> Url {
        Url::parse("https://example.com:443/private?token=secret").expect("valid target")
    }

    fn allowlisted_policy(target: &Url) -> ProxyPolicy {
        ProxyPolicy::deny_by_default()
            .with_allowed_target(target)
            .expect("target can be allowlisted")
    }

    #[test]
    fn deny_by_default_requires_explicit_authorization() {
        let policy = allowlisted_policy(&public_target());
        assert_eq!(
            policy
                .validate_target(&public_target(), ProxyAuthorization::None)
                .expect_err("unauthorized target must be denied"),
            ProxyPolicyError::AuthorizationRequired
        );
    }

    #[test]
    fn allowlisted_public_target_and_dns_answers_are_accepted() {
        let target = public_target();
        let policy = allowlisted_policy(&target);
        let validated = policy
            .validate_resolved_target(
                &target,
                ProxyAuthorization::Allowlisted,
                &[SocketAddr::from(([1, 1, 1, 1], 443))],
            )
            .expect("public target should pass");
        assert_eq!(validated.target().host(), "example.com");
        assert_eq!(validated.target().port(), 443);
        assert_eq!(
            validated.addresses(),
            &[SocketAddr::from(([1, 1, 1, 1], 443))]
        );
    }

    #[test]
    fn private_loopback_and_mapped_addresses_are_denied_by_default() {
        let target = Url::parse("http://internal.example:80").expect("valid target");
        let policy = ProxyPolicy::deny_by_default()
            .with_allowed_target(&target)
            .expect("target can be allowlisted");
        for address in [
            SocketAddr::from(([10, 0, 0, 1], 80)),
            SocketAddr::from(([127, 0, 0, 1], 80)),
            SocketAddr::new(Ipv6Addr::LOCALHOST.into(), 80),
            SocketAddr::new(
                Ipv6Addr::from([0, 0, 0, 0, 0, 0xffff, 0x7f00, 0x1]).into(),
                80,
            ),
        ] {
            assert_eq!(
                policy
                    .validate_resolved_target(&target, ProxyAuthorization::Allowlisted, &[address],)
                    .expect_err("special-use address must be denied"),
                ProxyPolicyError::AddressNotAllowed
            );
        }
    }

    #[test]
    fn explicit_local_opt_in_still_requires_authorization_and_keeps_reserved_denied() {
        let target = Url::parse("http://127.0.0.1:80").expect("valid target");
        let policy = ProxyPolicy::deny_by_default()
            .with_local_targets(true)
            .with_allowed_target(&target)
            .expect("target can be allowlisted");
        assert!(
            policy
                .validate_resolved_target(
                    &target,
                    ProxyAuthorization::Allowlisted,
                    &[SocketAddr::from(([127, 0, 0, 1], 80))],
                )
                .is_ok()
        );
        assert_eq!(
            policy
                .validate_resolved_target(
                    &Url::parse("http://reserved.example:80").expect("valid target"),
                    ProxyAuthorization::UserConfirmed,
                    &[SocketAddr::from(([192, 0, 2, 10], 80))],
                )
                .expect_err("documentation address is never a local opt-in"),
            ProxyPolicyError::UserConfirmationRequired
        );
    }

    #[test]
    fn user_confirmation_is_an_explicit_opt_in() {
        let target = public_target();
        let policy = ProxyPolicy::deny_by_default().with_user_confirmation(true);
        assert!(
            policy
                .validate_target(&target, ProxyAuthorization::UserConfirmed)
                .is_ok()
        );
    }

    #[test]
    fn dns_answer_sets_are_all_checked_and_retries_are_pinned() {
        let target = public_target();
        let policy = ProxyPolicy::deny_by_default().with_user_confirmation(true);
        let resolved = policy
            .validate_resolved_target(
                &target,
                ProxyAuthorization::UserConfirmed,
                &[SocketAddr::from(([1, 1, 1, 1], 443))],
            )
            .expect("public DNS answer should pass");
        assert!(
            policy
                .validate_retry(&resolved, SocketAddr::from(([1, 1, 1, 1], 443)))
                .is_ok()
        );
        assert_eq!(
            policy
                .validate_retry(&resolved, SocketAddr::from(([8, 8, 8, 8], 443)))
                .expect_err("new DNS answer must not be used for retry"),
            ProxyPolicyError::DnsRebindingDetected
        );
        assert_eq!(
            policy
                .validate_resolved_target(
                    &target,
                    ProxyAuthorization::UserConfirmed,
                    &[
                        SocketAddr::from(([1, 1, 1, 1], 443)),
                        SocketAddr::from(([127, 0, 0, 1], 443)),
                    ],
                )
                .expect_err("mixed public/private DNS answer must be denied"),
            ProxyPolicyError::AddressNotAllowed
        );
    }

    #[test]
    fn redirects_are_same_origin_bounded_and_never_downgrade_by_default() {
        let original_url = public_target();
        let policy = allowlisted_policy(&original_url);
        let original = policy
            .validate_target(&original_url, ProxyAuthorization::Allowlisted)
            .expect("original target should pass");
        let same_origin = Url::parse("https://EXAMPLE.com:443/next").expect("valid redirect");
        assert!(
            policy
                .validate_redirect(&original, &same_origin, ProxyAuthorization::Allowlisted, 0,)
                .is_ok()
        );
        assert_eq!(
            policy
                .validate_redirect(
                    &original,
                    &Url::parse("https://other.example:443/").expect("valid redirect"),
                    ProxyAuthorization::UserConfirmed,
                    0,
                )
                .expect_err("cross-origin redirect must be denied"),
            ProxyPolicyError::UserConfirmationRequired
        );
        assert_eq!(
            policy
                .validate_redirect(
                    &original,
                    &Url::parse("http://example.com:80/").expect("valid redirect"),
                    ProxyAuthorization::Allowlisted,
                    0,
                )
                .expect_err("HTTPS downgrade must be denied"),
            ProxyPolicyError::TargetNotAllowlisted
        );
    }

    #[test]
    fn redirect_policy_reports_specific_cross_origin_and_downgrade_errors() {
        let original_url = Url::parse("https://example.com:443/").expect("valid target");
        let policy = ProxyPolicy::deny_by_default().with_user_confirmation(true);
        let original = policy
            .validate_target(&original_url, ProxyAuthorization::UserConfirmed)
            .expect("original target should pass");
        assert_eq!(
            policy
                .validate_redirect(
                    &original,
                    &Url::parse("https://other.example:443/").expect("valid redirect"),
                    ProxyAuthorization::UserConfirmed,
                    0,
                )
                .expect_err("cross-origin redirect must be denied"),
            ProxyPolicyError::CrossOriginRedirect
        );
        assert_eq!(
            policy
                .validate_redirect(
                    &original,
                    &Url::parse("http://example.com:80/").expect("valid redirect"),
                    ProxyAuthorization::UserConfirmed,
                    0,
                )
                .expect_err("HTTPS downgrade must be denied"),
            ProxyPolicyError::InsecureRedirect
        );
        let no_redirects = ProxyPolicy::deny_by_default()
            .with_user_confirmation(true)
            .with_limits(ProxyResourceLimits {
                max_redirects: 0,
                ..ProxyResourceLimits::default()
            })
            .expect("zero redirects is a valid limit");
        let original = no_redirects
            .validate_target(&original_url, ProxyAuthorization::UserConfirmed)
            .expect("original target should pass");
        assert_eq!(
            no_redirects
                .validate_redirect(
                    &original,
                    &original_url,
                    ProxyAuthorization::UserConfirmed,
                    0,
                )
                .expect_err("redirect limit must be enforced"),
            ProxyPolicyError::RedirectLimitExceeded
        );
    }

    #[test]
    fn resource_limits_and_error_mapping_are_deterministic() {
        let policy = ProxyPolicy::deny_by_default()
            .with_limits(ProxyResourceLimits {
                max_request_bytes: 4,
                max_response_bytes: 8,
                max_concurrent_streams: 2,
                max_buffered_bytes: 16,
                ..ProxyResourceLimits::default()
            })
            .expect("limits should be valid");
        assert!(policy.check_request_size(4).is_ok());
        assert_eq!(
            policy.check_request_size(5),
            Err(ProxyPolicyError::RequestLimitExceeded)
        );
        assert_eq!(
            policy.check_response_size(9),
            Err(ProxyPolicyError::ResponseLimitExceeded)
        );
        assert_eq!(
            policy.check_concurrent_streams(2),
            Err(ProxyPolicyError::ConcurrencyLimitExceeded)
        );
        assert_eq!(
            policy.check_buffered_bytes(17),
            Err(ProxyPolicyError::BackpressureLimitExceeded)
        );
        assert_eq!(
            BlnkError::from(ProxyPolicyError::AddressNotAllowed).to_string(),
            "stream error: proxy policy rejected: address_not_allowed"
        );
        assert_eq!(
            ProxyPolicyError::AddressNotAllowed.code(),
            "address_not_allowed"
        );
    }

    #[test]
    fn malformed_targets_and_ports_are_rejected() {
        let policy = ProxyPolicy::deny_by_default();
        assert_eq!(
            policy
                .validate_target(
                    &Url::parse("https://user:pass@example.com/").expect("URL parses"),
                    ProxyAuthorization::UserConfirmed,
                )
                .expect_err("credentials must not enter proxy target URLs"),
            ProxyPolicyError::CredentialsNotAllowed
        );
        assert_eq!(
            policy
                .validate_target(
                    &Url::parse("tcp://example.com/").expect("URL parses"),
                    ProxyAuthorization::UserConfirmed,
                )
                .expect_err("TCP must have an explicit port"),
            ProxyPolicyError::InvalidPort
        );
        assert_eq!(
            policy
                .validate_target(
                    &Url::parse("https://example.com/#fragment").expect("URL parses"),
                    ProxyAuthorization::UserConfirmed,
                )
                .expect_err("fragments are not sent to network targets"),
            ProxyPolicyError::FragmentNotAllowed
        );
    }

    #[test]
    fn log_and_header_redaction_never_exposes_credentials_or_query_tokens() {
        let target = Url::parse("https://user:pass@example.com/private?token=secret#fragment")
            .expect("URL parses");
        let safe = redact_target_for_log(&target);
        assert_eq!(safe, "https://example.com:443");
        assert!(!safe.contains("secret"));
        assert_eq!(
            redact_header_value("Authorization", "Bearer secret"),
            REDACTED_VALUE
        );
        assert_eq!(
            redact_header_value("Content-Type", "text/plain"),
            "text/plain"
        );
    }
}
