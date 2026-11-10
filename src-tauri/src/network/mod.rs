//! Network: adapters and configuration, DNS changes, DNS benchmark, diagnostics.

pub mod dns;

use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::engine::types::{JobRequest, Shell};
use crate::error::{AppError, AppResult};
use crate::util::powershell;
use crate::util::serde_helpers::one_or_many;

const INFO: &str = include_str!("../../scripts/network-info.ps1");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    pub index: u32,
    pub name: String,
    pub description: String,
    pub status: String,
    pub media_type: String,
    pub physical: bool,
    pub link_speed: String,
    pub mac: String,
    #[serde(deserialize_with = "one_or_many")]
    pub ipv4: Vec<String>,
    #[serde(deserialize_with = "one_or_many")]
    pub ipv6: Vec<String>,
    pub gateway: String,
    pub dhcp: bool,
    pub mtu: u32,
    #[serde(deserialize_with = "one_or_many")]
    pub dns: Vec<String>,
    pub dns_manual: bool,
    pub category: Option<String>,
    pub network_name: Option<String>,
    pub ssid: Option<String>,
    pub signal: Option<u32>,
    pub radio: Option<String>,
    pub band: Option<String>,
    pub channel: Option<u32>,
}

pub fn adapters() -> AppResult<Vec<Adapter>> {
    #[derive(Deserialize)]
    struct W(#[serde(deserialize_with = "one_or_many")] Vec<Adapter>);
    let W(a) = powershell::run_json(INFO, Duration::from_secs(60))?;
    Ok(a)
}

fn err(m: impl Into<String>) -> AppError {
    AppError::Other(m.into())
}

fn find(index: u32) -> AppResult<Adapter> {
    adapters()?.into_iter().find(|a| a.index == index).ok_or_else(|| err("That network adapter no longer exists."))
}

fn job(title: impl Into<String>, op: &str, script: String) -> JobRequest {
    let mut r = JobRequest::internal(title, Shell::PowerShell, script);
    r.operation_id = Some(op.into());
    r.requires_admin = true;
    r.timeout = Duration::from_secs(120);
    r
}

/// Sets DNS servers on one adapter, or goes back to automatic (DHCP) when `servers` is empty.
/// Every address is parsed as an IP before it reaches the script.
pub fn set_dns(index: u32, servers: &[String]) -> AppResult<JobRequest> {
    let a = find(index)?;
    let mut parsed: Vec<IpAddr> = Vec::new();
    for s in servers.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        parsed.push(s.parse().map_err(|_| err(format!("'{s}' is not a valid IP address.")))?);
    }
    if parsed.len() > 4 {
        return Err(err("Use at most 4 DNS servers."));
    }
    let script = if parsed.is_empty() {
        format!("Set-DnsClientServerAddress -InterfaceIndex {index} -ResetServerAddresses -ErrorAction Stop\nClear-DnsClientCache\n'DNS on {name} now comes from your router (automatic).'", name = a.name.replace('\'', "''"))
    } else {
        let list = parsed.iter().map(|p| format!("'{p}'")).collect::<Vec<_>>().join(",");
        format!(
            "Set-DnsClientServerAddress -InterfaceIndex {index} -ServerAddresses @({list}) -ErrorAction Stop\nClear-DnsClientCache\n'DNS on {name} set to {plain}.'",
            name = a.name.replace('\'', "''"),
            plain = parsed.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ")
        )
    };
    Ok(job(format!("Set DNS on {}", a.name), "network.dns", script))
}

pub fn set_adapter_enabled(index: u32, enabled: bool) -> AppResult<JobRequest> {
    let a = find(index)?;
    let n = a.name.replace('\'', "''");
    let script = if enabled {
        format!("Enable-NetAdapter -Name '{n}' -Confirm:$false -ErrorAction Stop\n'{n} enabled.'")
    } else {
        format!("Disable-NetAdapter -Name '{n}' -Confirm:$false -ErrorAction Stop\n'{n} disabled. Turn it back on here when you need it.'")
    };
    Ok(job(format!("{} {}", if enabled { "Enable" } else { "Disable" }, a.name), "network.adapter", script))
}

pub fn set_category(index: u32, private: bool) -> AppResult<JobRequest> {
    let a = find(index)?;
    let cat = if private { "Private" } else { "Public" };
    let script = format!("Set-NetConnectionProfile -InterfaceIndex {index} -NetworkCategory {cat} -ErrorAction Stop\n'{} is now a {cat} network.'", a.name.replace('\'', "''"));
    Ok(job(format!("Set {} to {cat}", a.name), "network.category", script))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: &'static str,
    pub label: &'static str,
    pub ok: bool,
    pub detail: String,
    pub advice: Option<&'static str>,
}

fn tcp_ms(addr: SocketAddr, timeout: Duration) -> Option<u128> {
    let t = Instant::now();
    TcpStream::connect_timeout(&addr, timeout).ok().map(|_| t.elapsed().as_millis())
}

/// Step-by-step connectivity check, from the adapter outward.
pub fn diagnose() -> AppResult<Vec<Check>> {
    let all = adapters()?;
    let up: Vec<&Adapter> = all.iter().filter(|a| a.status == "Up" && a.physical).collect();
    let mut checks = Vec::new();
    let primary = up.iter().find(|a| !a.gateway.is_empty()).copied().or(up.first().copied());

    checks.push(Check {
        id: "adapter",
        label: "Network adapter connected",
        ok: primary.is_some(),
        detail: primary.map(|a| format!("{} ({}, {})", a.name, a.description, a.link_speed)).unwrap_or_else(|| "No physical adapter is connected.".into()),
        advice: Some("Check the cable or Wi-Fi connection, and make sure the adapter isn't disabled below."),
    });
    let Some(a) = primary else { return Ok(checks) };

    let has_ip = a.ipv4.iter().any(|ip| !ip.starts_with("169.254."));
    checks.push(Check {
        id: "ip",
        label: "Got an IP address",
        ok: has_ip,
        detail: a.ipv4.join(", "),
        advice: Some("A 169.254.x.x address means the router's DHCP didn't answer. Try 'Release and renew IP address' or restart the router."),
    });

    let gw_ok = !a.gateway.is_empty()
        && powershell::run(&format!("if (Test-Connection -ComputerName '{}' -Count 2 -Quiet) {{ 'ok' }}", a.gateway), Duration::from_secs(15))
            .map(|o| o.contains("ok"))
            .unwrap_or(false);
    checks.push(Check {
        id: "gateway",
        label: "Router reachable",
        ok: gw_ok,
        detail: if a.gateway.is_empty() { "No default gateway".into() } else { a.gateway.clone() },
        advice: Some("Restart the router, or move closer to it on Wi-Fi."),
    });

    let internet = tcp_ms(SocketAddr::from(([1, 1, 1, 1], 443)), Duration::from_secs(4)).or_else(|| tcp_ms(SocketAddr::from(([8, 8, 8, 8], 443)), Duration::from_secs(4)));
    checks.push(Check {
        id: "internet",
        label: "Internet reachable",
        ok: internet.is_some(),
        detail: internet.map(|ms| format!("Connected to a public server in {ms} ms")).unwrap_or_else(|| "Public servers did not answer".into()),
        advice: Some("The router is reachable but the internet isn't: check the modem or your provider's status."),
    });

    let dns_server = a.dns.iter().find_map(|d| d.parse::<IpAddr>().ok().filter(|ip| ip.is_ipv4()));
    let dns = dns_server.and_then(|s| dns::query_ms(s, "www.microsoft.com", Duration::from_secs(3)));
    let resolve = ("www.microsoft.com", 443).to_socket_addrs().map(|mut i| i.next().is_some()).unwrap_or(false);
    checks.push(Check {
        id: "dns",
        label: "Name lookup (DNS) works",
        ok: dns.is_some() || resolve,
        detail: match (dns_server, dns) {
            (Some(s), Some(ms)) => format!("{s} answered in {ms:.0} ms"),
            (Some(s), None) => format!("{s} did not answer"),
            _ => "No DNS server configured".into(),
        },
        advice: Some("Try a public DNS server (for example Cloudflare 1.1.1.1) below, then flush the DNS cache."),
    });
    Ok(checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapters_and_diagnostics() {
        let a = adapters().expect("adapters");
        assert!(a.iter().any(|x| x.physical), "{a:?}");
        let checks = diagnose().expect("diagnose");
        for c in &checks {
            println!("{} {}: {}", if c.ok { "OK " } else { "BAD" }, c.label, c.detail);
        }
        assert!(checks.iter().any(|c| c.id == "adapter"));
    }

    #[test]
    fn dns_input_is_validated() {
        let idx = adapters().unwrap().first().map(|a| a.index).unwrap_or(1);
        for bad in ["1.1.1.1; rm", "999.1.1.1", "dns.google", "'", "1.1.1.1 & x"] {
            assert!(set_dns(idx, &[bad.into()]).is_err(), "{bad}");
        }
        let r = set_dns(idx, &["1.1.1.1".into(), " 2606:4700:4700::1111 ".into()]).unwrap();
        assert!(r.command.contains("'1.1.1.1','2606:4700:4700::1111'"), "{}", r.command);
        assert!(r.requires_admin);
        assert!(set_dns(idx, &[]).unwrap().command.contains("-ResetServerAddresses"));
        assert!(set_dns(999_999, &["1.1.1.1".into()]).is_err());
    }

    #[test]
    fn dns_benchmark_runs() {
        let r = dns::benchmark(&[]);
        for x in &r {
            println!("{:<12} {:>6.1?} ms cached / {:>6.1?} ms uncached, {} failures", x.name, x.cached_ms, x.uncached_ms, x.failures);
        }
        assert_eq!(r.len(), dns::RESOLVERS.len());
        assert!(r.iter().any(|x| x.cached_ms.is_some()));
    }
}
