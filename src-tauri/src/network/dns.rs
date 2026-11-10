//! DNS benchmark: times real A-record queries sent over UDP to each resolver.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use serde::Serialize;

/// Well-known public resolvers offered as presets.
pub const RESOLVERS: &[(&str, &str, &str)] = &[
    ("Cloudflare", "1.1.1.1", "1.0.0.1"),
    ("Google", "8.8.8.8", "8.8.4.4"),
    ("Quad9", "9.9.9.9", "149.112.112.112"),
    ("OpenDNS", "208.67.222.222", "208.67.220.220"),
    ("AdGuard", "94.140.14.14", "94.140.15.15"),
];

const DOMAINS: &[&str] = &[
    "google.com", "youtube.com", "microsoft.com", "store.steampowered.com", "riotgames.com", "discord.com", "amazon.com", "wikipedia.org", "twitch.tv", "epicgames.com",
];

fn build_query(id: u16, name: &str) -> Vec<u8> {
    let mut q = Vec::with_capacity(32 + name.len());
    q.extend_from_slice(&id.to_be_bytes());
    q.extend_from_slice(&[0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]); // recursion desired, 1 question
    for label in name.split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.extend_from_slice(&[0, 0, 1, 0, 1]); // root, type A, class IN
    q
}

/// Milliseconds until a matching answer arrives (any response code counts as "answered").
pub fn query_ms(server: IpAddr, name: &str, timeout: Duration) -> Option<f64> {
    let bind: SocketAddr = if server.is_ipv4() { "0.0.0.0:0".parse().ok()? } else { "[::]:0".parse().ok()? };
    let sock = UdpSocket::bind(bind).ok()?;
    sock.set_read_timeout(Some(timeout)).ok()?;
    // Query IDs only need to differ between concurrent queries; wrapping arithmetic avoids overflow.
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    let id: u16 = (nanos as u16) ^ (name.len() as u16).wrapping_mul(7919) ^ (std::process::id() as u16);
    let packet = build_query(id, name);
    let start = Instant::now();
    sock.send_to(&packet, SocketAddr::new(server, 53)).ok()?;
    let mut buf = [0u8; 1500];
    loop {
        let (n, from) = sock.recv_from(&mut buf).ok()?;
        if from.ip() == server && n >= 12 && buf[0..2] == id.to_be_bytes() && buf[2] & 0x80 != 0 {
            return Some(start.elapsed().as_secs_f64() * 1000.0);
        }
        if start.elapsed() > timeout {
            return None;
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchResult {
    pub name: String,
    pub server: String,
    pub current: bool,
    /// Median of uncached lookups (first query per domain).
    pub uncached_ms: Option<f64>,
    /// Median of repeated (cached at the resolver) lookups.
    pub cached_ms: Option<f64>,
    pub failures: u32,
    pub queries: u32,
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(v[v.len() / 2])
}

/// Benchmarks the given resolvers in parallel: each domain is queried twice (first is likely
/// uncached at that resolver, second measures its cached response time).
pub fn benchmark(current: &[IpAddr]) -> Vec<BenchResult> {
    let mut targets: Vec<(String, IpAddr, bool)> = current.iter().filter(|ip| ip.is_ipv4()).map(|ip| (format!("Current ({ip})"), *ip, true)).collect();
    for (name, primary, _) in RESOLVERS {
        let ip: IpAddr = primary.parse().expect("valid preset IP");
        if !targets.iter().any(|(_, t, _)| *t == ip) {
            targets.push((name.to_string(), ip, false));
        }
    }
    let handles: Vec<_> = targets
        .into_iter()
        .map(|(name, ip, current)| {
            std::thread::spawn(move || {
                let (mut first, mut second, mut failures) = (Vec::new(), Vec::new(), 0u32);
                for d in DOMAINS {
                    match query_ms(ip, d, Duration::from_millis(1500)) {
                        Some(ms) => first.push(ms),
                        None => failures += 1,
                    }
                    match query_ms(ip, d, Duration::from_millis(1500)) {
                        Some(ms) => second.push(ms),
                        None => failures += 1,
                    }
                }
                BenchResult { name, server: ip.to_string(), current, uncached_ms: median(first), cached_ms: median(second), failures, queries: DOMAINS.len() as u32 * 2 }
            })
        })
        .collect();
    let mut out: Vec<BenchResult> = handles.into_iter().filter_map(|h| h.join().ok()).collect();
    out.sort_by(|a, b| {
        let score = |r: &BenchResult| r.cached_ms.unwrap_or(f64::MAX) + r.failures as f64 * 1000.0;
        score(a).partial_cmp(&score(b)).unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_packet_layout() {
        let q = build_query(0xABCD, "a.bc");
        assert_eq!(&q[..4], &[0xAB, 0xCD, 0x01, 0x00]);
        assert_eq!(&q[12..], &[1, b'a', 2, b'b', b'c', 0, 0, 1, 0, 1]);
    }

    #[test]
    fn real_resolver_answers() {
        let ms = query_ms("1.1.1.1".parse().unwrap(), "microsoft.com", Duration::from_secs(3));
        println!("1.1.1.1: {ms:?} ms");
        assert!(ms.is_some(), "Cloudflare DNS did not answer (offline?)");
    }
}
