use anyhow::{bail, Context, Result};
use chrono::Local;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::{env, fs};

// Keys written by this tool — stripped and replaced on every run.
// EXIT_IPV4 and EXIT_IPV6 are NOT in this list: they are user choices
// and must survive a re-run of import-wireguard.
const WG_KEYS: &[&str] = &[
    "EXIT_PRIVATE_KEY",
    "EXIT_TUNNEL_IPV4",
    "EXIT_TUNNEL_IPV6",
    "EXIT_MTU",
    "EXIT_SERVER_PUBKEY",
    "EXIT_PRESHARED_KEY",
    "EXIT_ENDPOINT_IP",
    "EXIT_ENDPOINT_PORT",
    "EXIT_KEEPALIVE",
];

const TEMPLATE: &str = "\n\
# Network — fill in manually\n\
UPSTREAM_SSID=\n\
UPSTREAM_WIFI_PASSWORD=\n\
TRAVEL_SSID=\n\
TRAVEL_WIFI_PASSWORD=\n\
NEXTDNS_PROFILE_ID=\n\
NEXTDNS_DEVICE_NAME=\n\
DEVICE_NAME=\n\
WG_LISTEN_PORT=13231\n\
LAN_SUBNET=192.168.88.0/24\n\
LAN_ULA_PREFIX=fd88:1::1/64\n\
EXIT_IPV4=yes\n\
EXIT_IPV6=yes\n";

fn parse_addresses(addr: &str) -> Result<(String, String)> {
    let mut ipv4 = String::new();
    let mut ipv6 = String::new();
    for part in addr.split(',') {
        let part = part.trim();
        if part.contains(':') {
            ipv6 = part.to_string();
        } else {
            ipv4 = part.to_string();
        }
    }
    if ipv6.is_empty() {
        bail!("Address field contains no IPv6 address");
    }
    Ok((ipv4, ipv6))
}

fn parse_endpoint(ep: &str) -> Result<(String, String)> {
    let ep = ep.trim();
    match ep.rfind(':') {
        Some(pos) => Ok((ep[..pos].to_string(), ep[pos + 1..].to_string())),
        None => bail!("Endpoint does not contain a port: {}", ep),
    }
}

fn require<'a>(conf: &'a HashMap<String, String>, key: &str) -> Result<&'a str> {
    conf.get(key)
        .map(String::as_str)
        .filter(|v| !v.is_empty())
        .with_context(|| format!("WireGuard config is missing required field: {}", key))
}

fn main() -> Result<()> {
    let env_path = env::args().nth(1).unwrap_or_else(|| ".env".to_string());

    let mut conf: HashMap<String, String> = HashMap::new();
    for line in io::stdin().lock().lines() {
        let line = line?;
        let line = line.trim().to_string();
        if line.starts_with('#') || line.starts_with('[') || line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            conf.insert(k.trim().to_string(), v.trim().trim_end_matches('\r').to_string());
        }
    }

    let private_key  = require(&conf, "PrivateKey")?;
    let address_raw  = require(&conf, "Address")?;
    let public_key   = require(&conf, "PublicKey")?;
    let preshared    = require(&conf, "PresharedKey")?;
    let endpoint_raw = require(&conf, "Endpoint")?;
    let mtu          = conf.get("MTU").map(String::as_str).unwrap_or("1320");
    let keepalive    = conf.get("PersistentKeepalive").map(String::as_str).unwrap_or("15");

    let (tunnel_ipv4, tunnel_ipv6) = parse_addresses(address_raw)?;
    let (endpoint_ip, endpoint_port) = parse_endpoint(endpoint_raw)?;

    let preserved: String = if fs::metadata(&env_path).is_ok() {
        let date = Local::now().format("%Y%m%d").to_string();
        let backup_path = (1u32..)
            .map(|n| format!("{}-{}-{:02}", env_path, date, n))
            .find(|p| fs::metadata(p).is_err())
            .expect("infallible");
        fs::rename(&env_path, &backup_path)?;
        eprintln!("backed up to {}", backup_path);
        fs::read_to_string(&backup_path)?
            .lines()
            .filter(|l| {
                !l.starts_with("# EXIT_* values") &&
                !WG_KEYS.iter().any(|k| l.starts_with(k))
            })
            .map(|l| format!("{}\n", l))
            .collect()
    } else {
        TEMPLATE.to_string()
    };

    let tmp_path = format!("{}.tmp", env_path);
    let mut f = fs::File::create(&tmp_path)?;
    writeln!(f, "# EXIT_* values — populated by import-wireguard")?;
    writeln!(f, "EXIT_PRIVATE_KEY={}", private_key)?;
    writeln!(f, "EXIT_TUNNEL_IPV4={}", tunnel_ipv4)?;
    writeln!(f, "EXIT_TUNNEL_IPV6={}", tunnel_ipv6)?;
    writeln!(f, "EXIT_MTU={}", mtu)?;
    writeln!(f, "EXIT_SERVER_PUBKEY={}", public_key)?;
    writeln!(f, "EXIT_PRESHARED_KEY={}", preshared)?;
    writeln!(f, "EXIT_ENDPOINT_IP={}", endpoint_ip)?;
    writeln!(f, "EXIT_ENDPOINT_PORT={}", endpoint_port)?;
    writeln!(f, "EXIT_KEEPALIVE={}", keepalive)?;
    write!(f, "{}", preserved)?;

    fs::rename(&tmp_path, &env_path)?;
    eprintln!("{} updated.", env_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn parse_addresses_ipv4_and_ipv6() {
        let (v4, v6) = parse_addresses("10.0.0.1/32, fd00::1/128").unwrap();
        assert_eq!(v4, "10.0.0.1/32");
        assert_eq!(v6, "fd00::1/128");
    }

    #[test]
    fn parse_addresses_ipv6_only() {
        let (v4, v6) = parse_addresses("fd00::1/128").unwrap();
        assert_eq!(v4, "");
        assert_eq!(v6, "fd00::1/128");
    }

    #[test]
    fn parse_addresses_no_ipv6_fails() {
        assert!(parse_addresses("10.0.0.1/32").is_err());
    }

    #[test]
    fn parse_endpoint_valid() {
        let (ip, port) = parse_endpoint("198.44.159.5:47107").unwrap();
        assert_eq!(ip, "198.44.159.5");
        assert_eq!(port, "47107");
    }

    #[test]
    fn parse_endpoint_no_port_fails() {
        assert!(parse_endpoint("198.44.159.5").is_err());
    }

    #[test]
    fn require_present() {
        let conf = HashMap::from([("Key".to_string(), "val".to_string())]);
        assert_eq!(require(&conf, "Key").unwrap(), "val");
    }

    #[test]
    fn require_missing_fails() {
        let conf = HashMap::new();
        assert!(require(&conf, "Missing").is_err());
    }

    #[test]
    fn require_empty_fails() {
        let conf = HashMap::from([("Key".to_string(), "".to_string())]);
        assert!(require(&conf, "Key").is_err());
    }
}
