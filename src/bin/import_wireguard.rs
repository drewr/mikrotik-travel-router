use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::{env, fs};

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
LAN_ULA_PREFIX=fd88:1::1/64\n";

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
        fs::read_to_string(&env_path)?
            .lines()
            .filter(|l| !l.starts_with("EXIT_") && !l.starts_with("# EXIT_"))
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
