#!/usr/bin/env bash
# wg-to-env.sh — Parse a WireGuard config from stdin and merge the
# extracted values into an env file, preserving any existing entries.
#
# Usage: cat airvpn.conf | bash wg-to-env.sh <env-file>
set -euo pipefail

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

[[ $# -eq 1 ]] || die "Usage: cat airvpn.conf | $0 <env-file>"
env_file="$1"

wg_conf=$(cat)

wg_get() {
    local key="$1"
    local val
    val=$(printf '%s\n' "$wg_conf" \
        | grep -m1 "^[[:space:]]*${key}[[:space:]]*=" \
        | sed 's/^[^=]*=[[:space:]]*//' \
        | tr -d '\r') || true
    printf '%s' "$val"
}

wg_require() {
    local key="$1"
    local val
    val=$(wg_get "$key")
    [[ -n "$val" ]] || die "WireGuard config is missing required field: $key"
    printf '%s' "$val"
}

private_key=$(wg_require PrivateKey)
address_raw=$(wg_require Address)
mtu=$(wg_get MTU)
public_key=$(wg_require PublicKey)
preshared_key=$(wg_require PresharedKey)
endpoint_raw=$(wg_require Endpoint)
keepalive=$(wg_get PersistentKeepalive)

tunnel_ipv4=$(printf '%s\n' "$address_raw" | tr ',' '\n' | tr -d ' \t' | grep -v ':' | head -1 || true)
tunnel_ipv6=$(printf '%s\n' "$address_raw" | tr ',' '\n' | tr -d ' \t' | grep  ':'  | head -1 || true)

[[ -n "$tunnel_ipv6" ]] || die "Address field contains no IPv6 address"

endpoint_port=$(printf '%s' "$endpoint_raw" | grep -oE '[0-9]+$' || true)
endpoint_ip=$(printf '%s' "$endpoint_raw" | sed "s/:${endpoint_port}$//")

[[ -n "$endpoint_port" ]] || die "Endpoint does not contain a port: $endpoint_raw"

tmp=$(mktemp)
{
    printf 'AIRVPN_PRIVATE_KEY=%s\n'   "$private_key"
    printf 'AIRVPN_TUNNEL_IPV4=%s\n'   "$tunnel_ipv4"
    printf 'AIRVPN_TUNNEL_IPV6=%s\n'   "$tunnel_ipv6"
    printf 'AIRVPN_MTU=%s\n'           "${mtu:-1320}"
    printf 'AIRVPN_SERVER_PUBKEY=%s\n' "$public_key"
    printf 'AIRVPN_PRESHARED_KEY=%s\n' "$preshared_key"
    printf 'AIRVPN_ENDPOINT_IP=%s\n'   "$endpoint_ip"
    printf 'AIRVPN_ENDPOINT_PORT=%s\n' "$endpoint_port"
    printf 'AIRVPN_KEEPALIVE=%s\n'     "${keepalive:-15}"
    if [[ -f "$env_file" ]]; then
        grep -v '^AIRVPN_' "$env_file" || true
    fi
} > "$tmp"
mv "$tmp" "$env_file"
printf '%s updated.\n' "$env_file" >&2
