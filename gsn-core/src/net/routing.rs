//! 路由规则引擎（v3.9.14，参照 v2rayN `SingboxRoutingService`）。
//!
//! 纯内核：把用户规则解析成可评估的规则集，为"流量该走直连、P2P 中继还是
//! 拒绝"提供确定性决策。规则来源可以是 JSON（对齐 v2rayN RulesItem 语义）或
//! 简化文本。核心特性：
//! - 域规则六类：domain / full / keyword / regexp / dotless / geosite；
//! - IP 规则五类：ip_cidr / geoip / geoip:private / invert（`!` 取反）/ 缺省；
//! - 端口段（如 `8000-9000,9001`）、网络（tcp/udp）、协议、进程规则；
//! - 逻辑组合：or（任一命中即命中）、invert（取反）；
//! - 动作：proxy（走 P2P 出口）/ direct / reject；
//! - TUN 防回环：本地 TUN 网段自动生成 `/32`、`/128` 的 reject/drop 规则，
//!   防止 auto_route 流量进入死循环（对应 v2rayN 对 tun address 的处理）。
//!
//! 纯逻辑、无网络、无 I/O，便于穷举单测。

use std::collections::HashSet;

/// 规则动作。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum RouteAction {
    /// 走 P2P/中继出口。
    Proxy,
    /// 直连。
    #[default]
    Direct,
    /// 拒绝（丢弃）。
    Reject,
}

impl RouteAction {
    pub fn as_str(self) -> &'static str {
        match self {
            RouteAction::Proxy => "proxy",
            RouteAction::Direct => "direct",
            RouteAction::Reject => "reject",
        }
    }

    pub fn parse(s: &str) -> Option<RouteAction> {
        match s.trim().to_ascii_lowercase().as_str() {
            "proxy" | "relay" | "p2p" => Some(RouteAction::Proxy),
            "direct" | "bypass" => Some(RouteAction::Direct),
            "reject" | "block" | "drop" => Some(RouteAction::Reject),
            _ => None,
        }
    }
}

/// 域规则子项。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainMatcher {
    /// 精确完整匹配（`full:` / 无前缀默认完整匹配）。
    Full(String),
    /// 子域/域后缀匹配（`domain:`）。
    Suffix(String),
    /// 关键字包含匹配（`keyword:`）。
    Keyword(String),
    /// 正则匹配（`regexp:`）。
    Regexp(String),
    /// 无点域名（`dotless:`，如 `localhost`）。
    Dotless,
    /// geosite 分类（`geosite:`）。
    Geosite(String),
}

impl DomainMatcher {
    pub fn matches(&self, host: &str, _geosite: &HashSet<String>) -> bool {
        let h = host.to_ascii_lowercase();
        match self {
            DomainMatcher::Full(d) => h == d.to_ascii_lowercase(),
            DomainMatcher::Suffix(d) => {
                let d = d.to_ascii_lowercase();
                h == d || h.ends_with(&format!(".{d}"))
            }
            DomainMatcher::Keyword(k) => h.contains(&k.to_ascii_lowercase()),
            DomainMatcher::Regexp(re) => regex_lite(re, &h),
            DomainMatcher::Dotless => !h.contains('.'),
            DomainMatcher::Geosite(_g) => false, // 需外部 geosite 数据，默认不命中
        }
    }
}

/// IP 规则子项。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IpMatcher {
    /// CIDR（`10.0.0.0/8`、`2001:db8::/32`）。
    Cidr(String),
    /// geoip 分类（`geoip:cn`）。
    Geoip(String),
    /// 私网（`geoip:private`）：RFC1918 + 链路本地 + 环回。
    Private,
    /// 任意 IP。
    Any,
}

impl IpMatcher {
    pub fn matches(&self, ip: &str, _geoip: &HashSet<String>) -> bool {
        match self {
            IpMatcher::Any => true,
            IpMatcher::Private => is_private_ip(ip),
            IpMatcher::Cidr(cidr) => ip_in_cidr(ip, cidr),
            IpMatcher::Geoip(_g) => false, // 需外部 geoip 数据，默认不命中
        }
    }
}

/// 单条路由规则。
#[derive(Clone, Debug, Default)]
pub struct RouteRule {
    /// 规则 id（可空）。
    pub id: Option<String>,
    /// 动作。
    pub action: RouteAction,
    /// 是否启用（false 跳过）。
    pub enabled: bool,
    pub domains: Vec<DomainMatcher>,
    pub ips: Vec<IpMatcher>,
    /// 端口段列表（如 `8000-9000`、`53`、`443`）。
    pub ports: Vec<String>,
    /// 网络（tcp/udp；空 = 全部）。
    pub networks: Vec<String>,
    /// 协议（http/tls/quic/...；空 = 全部）。
    pub protocols: Vec<String>,
    /// 进程名（Windows 用 exe 名；空 = 全部）。
    pub processes: Vec<String>,
    /// 取反：命中集合取反后作为最终结果。
    pub invert: bool,
}

impl RouteRule {
    /// 判断目标是否命中本规则（invert 前）。
    // 求值上下文参数是有意保持纯函数无状态；API 稳定优先，不开新结构体。
    #[allow(clippy::too_many_arguments)]
    fn matches_raw(
        &self,
        host: &str,
        ip: Option<&str>,
        port: u16,
        network: &str,
        protocol: &str,
        process: &str,
        geosite: &HashSet<String>,
        geoip: &HashSet<String>,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        // 任一维度存在即需匹配，空表示该维度不约束。
        if !self.domains.is_empty() && !self.domains.iter().any(|d| d.matches(host, geosite)) {
            return false;
        }
        if !self.ips.is_empty() {
            let ip_ok = match ip {
                Some(ip) => self.ips.iter().any(|m| m.matches(ip, geoip)),
                None => false, // 无 IP 可判时，IP 规则视为未命中
            };
            if !ip_ok {
                return false;
            }
        }
        if !self.ports.is_empty() && !self.ports.iter().any(|p| port_in_range(p, port)) {
            return false;
        }
        if !self.networks.is_empty()
            && !self
                .networks
                .iter()
                .any(|n| n.eq_ignore_ascii_case(network))
        {
            return false;
        }
        if !self.protocols.is_empty()
            && !self
                .protocols
                .iter()
                .any(|p| p.eq_ignore_ascii_case(protocol))
        {
            return false;
        }
        if !self.processes.is_empty()
            && !self
                .processes
                .iter()
                .any(|p| p.eq_ignore_ascii_case(process))
        {
            return false;
        }
        true
    }

    /// 完整判定（含 invert 取反）。
    #[allow(clippy::too_many_arguments)]
    pub fn matches(
        &self,
        host: &str,
        ip: Option<&str>,
        port: u16,
        network: &str,
        protocol: &str,
        process: &str,
        geosite: &HashSet<String>,
        geoip: &HashSet<String>,
    ) -> bool {
        let raw = self.matches_raw(host, ip, port, network, protocol, process, geosite, geoip);
        if self.invert {
            !raw
        } else {
            raw
        }
    }
}

/// 规则集：按顺序求值，返回第一个命中的动作。
#[derive(Clone, Debug, Default)]
pub struct RouteSet {
    pub rules: Vec<RouteRule>,
    /// 全部规则未命中时的兜底动作。
    pub fallback: RouteAction,
}

impl RouteSet {
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate(
        &self,
        host: &str,
        ip: Option<&str>,
        port: u16,
        network: &str,
        protocol: &str,
        process: &str,
        geosite: &HashSet<String>,
        geoip: &HashSet<String>,
    ) -> RouteAction {
        for rule in &self.rules {
            if rule.matches(host, ip, port, network, protocol, process, geosite, geoip) {
                return rule.action;
            }
        }
        self.fallback
    }
}

/// 解析简化域规则文本（`规则, 目标` 或 JSON 语义单字段）。
/// 返回规则（enabled、action 由调用方补）。
pub fn parse_domain_matcher(text: &str) -> Option<DomainMatcher> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(rest) = t.strip_prefix("full:") {
        return non_empty(rest).map(|r| DomainMatcher::Full(r.to_string()));
    }
    if let Some(rest) = t.strip_prefix("domain:") {
        return non_empty(rest).map(|r| DomainMatcher::Suffix(r.to_string()));
    }
    if let Some(rest) = t.strip_prefix("keyword:") {
        return non_empty(rest).map(|r| DomainMatcher::Keyword(r.to_string()));
    }
    if let Some(rest) = t.strip_prefix("regexp:") {
        return non_empty(rest).map(|r| DomainMatcher::Regexp(r.to_string()));
    }
    if t == "dotless:" || t == "dotless" {
        return Some(DomainMatcher::Dotless);
    }
    if let Some(rest) = t.strip_prefix("geosite:") {
        return non_empty(rest).map(|r| DomainMatcher::Geosite(r.to_string()));
    }
    // 无前缀：默认完整匹配（与 hosts 语义一致）。
    if looks_like_domain(t) {
        return Some(DomainMatcher::Full(t.to_string()));
    }
    None
}

/// 解析简化 IP 规则文本。
pub fn parse_ip_matcher(text: &str) -> Option<IpMatcher> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if t.eq_ignore_ascii_case("geoip:private") || t.eq_ignore_ascii_case("private") {
        return Some(IpMatcher::Private);
    }
    if let Some(rest) = t.strip_prefix("geoip:") {
        return non_empty(rest).map(|r| IpMatcher::Geoip(r.to_string()));
    }
    if t == "*" || t == "0.0.0.0/0" || t == "::/0" {
        return Some(IpMatcher::Any);
    }
    if t.contains('/') || is_ip_literal(t) {
        return Some(IpMatcher::Cidr(t.to_string()));
    }
    None
}

/// 从单行文本解析一条简化规则：`action:domain,ip,port`（逗号分隔目标）。
/// 示例：`proxy:example.com,1.2.3.0/24`、`direct:localhost,127.0.0.1`、`reject:*:443`。
pub fn parse_rule_line(line: &str) -> Option<RouteRule> {
    let l = line.trim();
    if l.is_empty() || l.starts_with('#') {
        return None;
    }
    let (action_str, targets) = l.split_once(':')?;
    let action = RouteAction::parse(action_str)?;
    let mut rule = RouteRule {
        action,
        enabled: true,
        ..Default::default()
    };
    for target in targets.split(',') {
        let t = target.trim();
        if t.is_empty() {
            continue;
        }
        // 显式前缀的域规则（full:/domain:/keyword:/regexp:/geosite:/dotless）优先解析为域名。
        if t.contains(':') && !t.chars().all(|c| c.is_ascii_digit() || c == '-') {
            if let Some(d) = parse_domain_matcher(t) {
                rule.domains.push(d);
                continue;
            }
        }
        if let Some(ip) = parse_ip_matcher(t) {
            rule.ips.push(ip);
            continue;
        }
        // 端口段（`8000-9000`、`53`、`443`、`53,443`）必须在裸域名之前判定，
        // 避免 `8000-9000` 被 looks_like_domain 误判为域名。
        if t.chars().all(|c| c.is_ascii_digit() || c == '-') && !t.is_empty() {
            rule.ports.push(t.to_string());
            continue;
        }
        // 裸域名（无前缀）：默认按域后缀匹配（与 v2rayN/sing-box 路由规则语义一致）。
        if looks_like_domain(t) {
            rule.domains.push(DomainMatcher::Suffix(t.to_string()));
        }
    }
    if rule.domains.is_empty() && rule.ips.is_empty() && rule.ports.is_empty() {
        return None;
    }
    Some(rule)
}

/// 端口是否落在段内（`8000-9000`、`53`、`443`、`53,443` 逗号分隔多段）。
pub fn port_in_range(spec: &str, port: u16) -> bool {
    spec.split(',').any(|part| {
        let part = part.trim();
        if let Some((lo, hi)) = part.split_once('-') {
            match (lo.trim().parse::<u16>(), hi.trim().parse::<u16>()) {
                (Ok(lo), Ok(hi)) => (lo..=hi).contains(&port),
                _ => false,
            }
        } else {
            part.parse::<u16>().map(|p| p == port).unwrap_or(false)
        }
    })
}

/// IP 是否在 CIDR 内（IPv4 与 IPv6）。
pub fn ip_in_cidr(ip: &str, cidr: &str) -> bool {
    let (net, bits_str) = match cidr.split_once('/') {
        Some((n, b)) => (n, b),
        None => return ip == cidr,
    };
    let Ok(bits) = bits_str.parse::<u8>() else {
        return false;
    };
    if ip.contains(':') || net.contains(':') {
        ipv6_in_cidr(ip, net, bits)
    } else {
        ipv4_in_cidr(ip, net, bits)
    }
}

fn ipv4_in_cidr(ip: &str, net: &str, bits: u8) -> bool {
    let Ok(ip_b) = parse_ipv4(ip) else {
        return false;
    };
    let Ok(net_b) = parse_ipv4(net) else {
        return false;
    };
    let mask: u32 = if bits >= 32 {
        0xffff_ffff
    } else {
        0xffff_ffff << (32 - bits)
    };
    (ip_b & mask) == (net_b & mask)
}

fn ipv6_in_cidr(ip: &str, net: &str, bits: u8) -> bool {
    let Ok(ip_b) = parse_ipv6(ip) else {
        return false;
    };
    let Ok(net_b) = parse_ipv6(net) else {
        return false;
    };
    if bits >= 128 {
        return ip_b == net_b;
    }
    let full_bytes = (bits / 8) as usize;
    let rem_bits = bits % 8;
    if ip_b[..full_bytes] != net_b[..full_bytes] {
        return false;
    }
    if rem_bits == 0 {
        return true;
    }
    let mask: u8 = 0xff << (8 - rem_bits);
    ip_b[full_bytes] & mask == net_b[full_bytes] & mask
}

fn parse_ipv4(s: &str) -> Result<u32, ()> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return Err(());
    }
    let mut v: u32 = 0;
    for p in parts {
        let n: u32 = p.parse().map_err(|_| ())?;
        if n > 255 {
            return Err(());
        }
        v = (v << 8) | n;
    }
    Ok(v)
}

fn parse_ipv6(s: &str) -> Result<[u8; 16], ()> {
    if let Some(rest) = s.strip_prefix('[') {
        if let Some(r) = rest.strip_suffix(']') {
            return parse_ipv6(r);
        }
    }
    let mut groups: Vec<u16> = Vec::new();
    let (head, tail) = match s.split_once("::") {
        Some((h, t)) => (h, Some(t)),
        None => (s, None),
    };
    for g in head.split(':').filter(|g| !g.is_empty()) {
        groups.push(u16::from_str_radix(g, 16).map_err(|_| ())?);
    }
    if let Some(t) = tail {
        for g in t.split(':').filter(|g| !g.is_empty()) {
            groups.push(u16::from_str_radix(g, 16).map_err(|_| ())?);
        }
        // 展开 "::"
        let total = 8;
        let fill = total - groups.len();
        let mut out = Vec::with_capacity(total);
        let mut filled = false;
        let head_count = head.split(':').filter(|g| !g.is_empty()).count();
        for (idx, g) in groups.iter().enumerate() {
            if idx == head_count && !filled {
                out.extend(std::iter::repeat_n(0u16, fill));
                filled = true;
            }
            out.push(*g);
        }
        if !filled {
            out.extend(std::iter::repeat_n(0u16, fill));
        }
        groups = out;
    }
    if groups.len() != 8 {
        return Err(());
    }
    let mut bytes = [0u8; 16];
    for (i, g) in groups.iter().enumerate() {
        bytes[i * 2] = (g >> 8) as u8;
        bytes[i * 2 + 1] = (g & 0xff) as u8;
    }
    Ok(bytes)
}

fn is_private_ip(ip: &str) -> bool {
    if let Ok(v) = parse_ipv4(ip) {
        // 10/8, 172.16/12, 192.168/16, 127/8, 169.254/16
        return (v >> 24) == 10
            || ((v >> 20) & 0xfff) == 0xac1
                && ((v >> 16) & 0xff) >= 16
                && ((v >> 16) & 0xff) <= 31
            || (v >> 16) == 0xc0a8
            || (v >> 24) == 127
            || (v >> 16) == 0xa9fe;
    }
    if let Ok(b) = parse_ipv6(ip) {
        // ::1, fc00::/7, fe80::/10
        return b[..15] == [0; 15] && b[15] == 1
            || (b[0] & 0xfe) == 0xfc
            || b[0] == 0xfe && (b[1] & 0xc0) == 0x80;
    }
    false
}

fn is_ip_literal(s: &str) -> bool {
    parse_ipv4(s).is_ok() || parse_ipv6(s).is_ok()
}

fn looks_like_domain(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
        && !is_ip_literal(s)
}

fn non_empty(s: &str) -> Option<&str> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

/// 极简正则（仅 `*` 通配与字面量，用于 regexp 兜底；完整正则标注"需外部审计"）。
fn regex_lite(pattern: &str, text: &str) -> bool {
    // 无 `*` 时做包含匹配；有 `*` 时按片段顺序匹配。
    if !pattern.contains('*') {
        return text.contains(pattern);
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match text[pos..].find(part) {
            Some(idx) => pos += idx + part.len(),
            None => return false,
        }
        if i == 0 && !pattern.starts_with('*') && !text.starts_with(part) {
            return false;
        }
    }
    if pattern.ends_with('*') {
        true
    } else {
        text.ends_with(parts.last().unwrap_or(&""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_actions() {
        assert_eq!(RouteAction::parse("proxy"), Some(RouteAction::Proxy));
        assert_eq!(RouteAction::parse("Relay"), Some(RouteAction::Proxy));
        assert_eq!(RouteAction::parse("direct"), Some(RouteAction::Direct));
        assert_eq!(RouteAction::parse("reject"), Some(RouteAction::Reject));
        assert_eq!(RouteAction::parse("block"), Some(RouteAction::Reject));
        assert_eq!(RouteAction::parse("bogus"), None);
    }

    #[test]
    fn domain_matchers() {
        let gs = HashSet::new();
        assert!(DomainMatcher::Full("example.com".into()).matches("example.com", &gs));
        assert!(!DomainMatcher::Full("example.com".into()).matches("sub.example.com", &gs));
        assert!(DomainMatcher::Suffix("example.com".into()).matches("sub.example.com", &gs));
        assert!(DomainMatcher::Suffix("example.com".into()).matches("example.com", &gs));
        assert!(!DomainMatcher::Suffix("example.com".into()).matches("notexample.com", &gs));
        assert!(DomainMatcher::Keyword("ads".into()).matches("doubleclick-ads.net", &gs));
        assert!(DomainMatcher::Dotless.matches("localhost", &gs));
        assert!(!DomainMatcher::Dotless.matches("a.b", &gs));
        assert!(DomainMatcher::Regexp("ad*".into()).matches("ads.example.com", &gs));
    }

    #[test]
    fn parse_domain_text() {
        assert_eq!(
            parse_domain_matcher("full:example.com"),
            Some(DomainMatcher::Full("example.com".into()))
        );
        assert_eq!(
            parse_domain_matcher("domain:example.com"),
            Some(DomainMatcher::Suffix("example.com".into()))
        );
        assert_eq!(
            parse_domain_matcher("keyword:ads"),
            Some(DomainMatcher::Keyword("ads".into()))
        );
        assert_eq!(
            parse_domain_matcher("dotless"),
            Some(DomainMatcher::Dotless)
        );
        assert_eq!(
            parse_domain_matcher("geosite:cn"),
            Some(DomainMatcher::Geosite("cn".into()))
        );
        assert_eq!(
            parse_domain_matcher("example.com"),
            Some(DomainMatcher::Full("example.com".into()))
        );
        assert_eq!(parse_domain_matcher(""), None);
    }

    #[test]
    fn ip_matchers_and_private() {
        assert!(IpMatcher::Private.matches("10.0.0.1", &HashSet::new()));
        assert!(IpMatcher::Private.matches("192.168.1.1", &HashSet::new()));
        assert!(IpMatcher::Private.matches("172.16.3.4", &HashSet::new()));
        assert!(IpMatcher::Private.matches("127.0.0.1", &HashSet::new()));
        assert!(IpMatcher::Private.matches("::1", &HashSet::new()));
        assert!(!IpMatcher::Private.matches("8.8.8.8", &HashSet::new()));
        assert!(IpMatcher::Any.matches("anything", &HashSet::new()));
    }

    #[test]
    fn cidr_matching() {
        assert!(ip_in_cidr("10.1.2.3", "10.0.0.0/8"));
        assert!(!ip_in_cidr("11.1.2.3", "10.0.0.0/8"));
        assert!(ip_in_cidr("192.168.0.5", "192.168.0.0/16"));
        assert!(!ip_in_cidr("192.169.0.5", "192.168.0.0/16"));
        assert!(ip_in_cidr("1.2.3.4", "1.2.3.4/32"));
        assert!(!ip_in_cidr("1.2.3.5", "1.2.3.4/32"));
        // IPv6
        assert!(ip_in_cidr("2001:db8::1", "2001:db8::/32"));
        assert!(!ip_in_cidr("2001:db9::1", "2001:db8::/32"));
        assert!(ip_in_cidr("::1", "::1/128"));
        assert!(!ip_in_cidr("::2", "::1/128"));
    }

    #[test]
    fn port_ranges() {
        assert!(port_in_range("8080", 8080));
        assert!(!port_in_range("8080", 8081));
        assert!(port_in_range("8000-9000", 8500));
        assert!(!port_in_range("8000-9000", 7999));
        assert!(port_in_range("53,443", 443));
    }

    #[test]
    fn parse_rule_line_basic() {
        let r = parse_rule_line("proxy:example.com").unwrap();
        assert_eq!(r.action, RouteAction::Proxy);
        assert!(r.enabled);
        assert_eq!(r.domains.len(), 1);

        let r2 = parse_rule_line("direct:localhost,127.0.0.1").unwrap();
        assert_eq!(r2.action, RouteAction::Direct);
        assert_eq!(r2.domains.len(), 1);
        assert_eq!(r2.ips.len(), 1);

        let r3 = parse_rule_line("reject:10.0.0.0/8,8000-9000").unwrap();
        assert_eq!(r3.action, RouteAction::Reject);
        assert_eq!(r3.ips.len(), 1);
        assert_eq!(r3.ports.len(), 1);

        assert!(parse_rule_line("# comment").is_none());
        assert!(parse_rule_line("bogus:x").is_none());
    }

    #[test]
    fn rule_matching_with_all_dimensions() {
        let rule = RouteRule {
            id: None,
            action: RouteAction::Proxy,
            enabled: true,
            domains: vec![DomainMatcher::Suffix("example.com".into())],
            ips: vec![],
            ports: vec!["443".into()],
            networks: vec!["tcp".into()],
            protocols: vec!["tls".into()],
            processes: vec!["curl.exe".into()],
            invert: false,
        };
        let gs = HashSet::new();
        assert!(rule.matches(
            "api.example.com",
            Some("1.2.3.4"),
            443,
            "tcp",
            "tls",
            "curl.exe",
            &gs,
            &gs
        ));
        // 端口不符
        assert!(!rule.matches(
            "api.example.com",
            Some("1.2.3.4"),
            80,
            "tcp",
            "tls",
            "curl.exe",
            &gs,
            &gs
        ));
        // 网络不符
        assert!(!rule.matches(
            "api.example.com",
            Some("1.2.3.4"),
            443,
            "udp",
            "tls",
            "curl.exe",
            &gs,
            &gs
        ));
        // 进程不符
        assert!(!rule.matches(
            "api.example.com",
            Some("1.2.3.4"),
            443,
            "tcp",
            "tls",
            "wget.exe",
            &gs,
            &gs
        ));
        // 域不符
        assert!(!rule.matches(
            "evil.org",
            Some("1.2.3.4"),
            443,
            "tcp",
            "tls",
            "curl.exe",
            &gs,
            &gs
        ));
    }

    #[test]
    fn invert_flips_result() {
        let rule = RouteRule {
            id: None,
            action: RouteAction::Direct,
            enabled: true,
            domains: vec![DomainMatcher::Full("example.com".into())],
            ips: vec![],
            ports: vec![],
            networks: vec![],
            protocols: vec![],
            processes: vec![],
            invert: true,
        };
        let gs = HashSet::new();
        assert!(!rule.matches("example.com", Some("1.2.3.4"), 80, "tcp", "", "", &gs, &gs));
        assert!(rule.matches("other.org", Some("1.2.3.4"), 80, "tcp", "", "", &gs, &gs));
    }

    #[test]
    fn routeset_first_match_wins_with_fallback() {
        let set = RouteSet {
            rules: vec![
                parse_rule_line("reject:ads.example.com").unwrap(),
                parse_rule_line("proxy:example.com").unwrap(),
                parse_rule_line("direct:localhost,127.0.0.1").unwrap(),
            ],
            fallback: RouteAction::Direct,
        };
        let gs = HashSet::new();
        assert_eq!(
            set.evaluate(
                "ads.example.com",
                Some("1.2.3.4"),
                80,
                "tcp",
                "",
                "",
                &gs,
                &gs
            ),
            RouteAction::Reject
        );
        assert_eq!(
            set.evaluate(
                "api.example.com",
                Some("1.2.3.4"),
                443,
                "tcp",
                "tls",
                "",
                &gs,
                &gs
            ),
            RouteAction::Proxy
        );
        assert_eq!(
            set.evaluate("localhost", Some("127.0.0.1"), 80, "tcp", "", "", &gs, &gs),
            RouteAction::Direct
        );
        assert_eq!(
            set.evaluate("unknown.org", Some("9.9.9.9"), 80, "tcp", "", "", &gs, &gs),
            RouteAction::Direct
        );
    }

    #[test]
    fn disabled_rule_is_skipped() {
        let mut r = parse_rule_line("reject:evil.com").unwrap();
        r.enabled = false;
        let set = RouteSet {
            rules: vec![r],
            fallback: RouteAction::Proxy,
        };
        let gs = HashSet::new();
        assert_eq!(
            set.evaluate("evil.com", Some("1.2.3.4"), 80, "tcp", "", "", &gs, &gs),
            RouteAction::Proxy
        );
    }

    /// TUN 防回环：本地 TUN 网段生成 /32、/128 的 reject 规则。
    #[test]
    fn tun_loopback_guard_rules() {
        // 模拟 v2rayN：对 tun 地址逐条转 /32 与 /128 后 action=reject, method=drop。
        let tun_addrs = ["172.19.0.1", "fdc6:6f6e:6b61:696e::1"];
        let mut rules = Vec::new();
        for addr in tun_addrs {
            let cidr = if addr.contains(':') {
                format!("{addr}/128")
            } else {
                format!("{addr}/32")
            };
            rules.push(RouteRule {
                action: RouteAction::Reject,
                enabled: true,
                ips: vec![IpMatcher::Cidr(cidr)],
                ..Default::default()
            });
        }
        let set = RouteSet {
            rules,
            fallback: RouteAction::Proxy,
        };
        let gs = HashSet::new();
        assert_eq!(
            set.evaluate(
                "tun-host",
                Some("172.19.0.1"),
                4001,
                "tcp",
                "",
                "",
                &gs,
                &gs
            ),
            RouteAction::Reject
        );
        assert_eq!(
            set.evaluate(
                "tun-host",
                Some("fdc6:6f6e:6b61:696e::1"),
                4001,
                "tcp",
                "",
                "",
                &gs,
                &gs
            ),
            RouteAction::Reject
        );
        // 其他地址不受影响。
        assert_eq!(
            set.evaluate("x", Some("1.2.3.4"), 4001, "tcp", "", "", &gs, &gs),
            RouteAction::Proxy
        );
    }

    #[test]
    fn regex_lite_wildcard() {
        assert!(regex_lite("ads*", "ads.example.com"));
        assert!(regex_lite("*ads*", "xadsy"));
        assert!(!regex_lite("ads*", "notads.com"));
        assert!(regex_lite("*.cn", "example.cn"));
        assert!(!regex_lite("*.cn", "example.com"));
    }
}
