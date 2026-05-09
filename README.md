# MikroTik hAP ax² Travel Router

A MikroTik hAP ax² (C52iG-5HaxD2HaxD) configured as a travel router with a
WireGuard exit node for IPv4 and IPv6. A Rust tool generates the RouterOS setup
script from a `.env` file so secrets never live in this repo.

```
  [Upstream WiFi AP]
         |
    (wifi1, 5GHz)                  ← station-bridge to upstream, WAN
         |
   [ MikroTik bridge ]
    /    |    \
  eth2  eth3  wifiTravel(5GHz vAP) ← LAN clients
  eth4  eth5
              wifi2(2.4GHz)        ← disabled (see issue #1 for fallback plan)

  IPv4 + IPv6 → WireGuard exit node (interface: exit)
```

---

## Prerequisites

- MikroTik hAP ax² factory fresh or reset to defaults
- Upstream WiFi credentials (hotel, office, home, etc.)
- WireGuard config downloaded from an IPv6-capable exit node provider
  (AirVPN or your favourite WireGuard VPN)
- Nix, or Rust toolchain (`cargo build --release`)
- (Optional) NextDNS account and profile ID

---

## Quickstart

### 1. Prepare the env file

```sh
cp .env.example .env
$EDITOR .env          # fill in the network values
```

Populate the `EXIT_*` values from your downloaded WireGuard config:

```sh
nix run .#import-wireguard < wireguard.conf
```

### 2. Generate and apply the RouterOS script

```sh
nix run .#generate > setup.rsc
scp -o StrictHostKeyChecking=no setup.rsc admin@192.168.88.1:/
ssh -o StrictHostKeyChecking=no admin@192.168.88.1 "/import file-name=setup.rsc"
```

Use `admin` (not `root`) — root doesn't exist until the RSC creates it. After
the import completes, install your SSH key for root:

```sh
nix run .#generate > /dev/null   # ROOT_SSH_PUBLIC_KEY_FILE must be set
```

Or manually:

```sh
ssh -4 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  root@192.168.88.1 \
  "/file print file=mykey.txt; \
   file set mykey.txt contents=\"$(cat ~/.ssh/id_ed25519.pub)\"; \
   /user ssh-keys import public-key-file=mykey.txt"
```

### 3. Install CA certificates

This must be done manually once, after wifi1 has an upstream connection, before
DoH verification will work:

```
/tool fetch url=https://curl.se/ca/cacert.pem
/certificate import file-name=cacert.pem
```

---

## Configuration reference

All variables are read from the environment. Copy `.env.example` to `.env` and
fill in values; `import-wireguard` populates the `EXIT_*` block automatically.

### EXIT_* — populated by `import-wireguard`

| Variable | Description |
|---|---|
| `EXIT_PRIVATE_KEY` | WireGuard private key |
| `EXIT_TUNNEL_IPV4` | Tunnel IPv4 address (CIDR) |
| `EXIT_TUNNEL_IPV6` | Tunnel IPv6 address (CIDR) |
| `EXIT_MTU` | Interface MTU (typically 1320) |
| `EXIT_SERVER_PUBKEY` | Exit node public key |
| `EXIT_PRESHARED_KEY` | WireGuard preshared key |
| `EXIT_ENDPOINT_IP` | Exit node IP address |
| `EXIT_ENDPOINT_PORT` | Exit node UDP port |
| `EXIT_KEEPALIVE` | Persistent keepalive in seconds |

### Network — fill in manually

| Variable | Default | Description |
|---|---|---|
| `UPSTREAM_SSID` | *(required)* | SSID of the upstream WiFi network |
| `UPSTREAM_WIFI_PASSWORD` | *(required)* | Upstream WiFi passphrase |
| `TRAVEL_SSID` | *(required)* | SSID of the local travel AP |
| `TRAVEL_WIFI_PASSWORD` | *(required)* | Travel AP passphrase |
| `NEXTDNS_PROFILE_ID` | *(required)* | NextDNS profile ID |
| `NEXTDNS_DEVICE_NAME` | value of `DEVICE_NAME` | Device name shown in NextDNS |
| `DEVICE_NAME` | *(required)* | RouterOS identity and hostname |
| `ROUTER_IP` | `192.168.88.1` | Router LAN IP (used for SSH key install) |
| `ROOT_SSH_PUBLIC_KEY_FILE` | *(unset)* | Path to public key to install on root |
| `LAN_SUBNET` | `192.168.88.0/24` | LAN subnet — change if upstream uses the same range |
| `LAN_ULA_PREFIX` | `fd88:1::1/64` | IPv6 ULA prefix for LAN |
| `WG_LISTEN_PORT` | `13231` | Local WireGuard UDP listen port |
| `EXIT_IPV4` | `yes` | Route IPv4 through the exit node |
| `EXIT_IPV6` | `yes` | Route IPv6 through the exit node |

#### EXIT_IPV4 and EXIT_IPV6

Setting either to `no` leaves that protocol routing directly through the upstream
ISP. At least one must be `yes`.

| EXIT_IPV4 | EXIT_IPV6 | WireGuard allowed-address |
|---|---|---|
| yes | yes | `0.0.0.0/0,::/0` |
| no | yes | `::/0` |
| yes | no | `0.0.0.0/0` |

When `EXIT_IPV4=yes` the generated RSC also installs a named RouterOS script
(`exit-endpoint-update`) on the DHCP client. It fires on every lease renewal and
updates the endpoint host route to use the current upstream gateway — so the tunnel
survives moving between networks.

#### LAN_SUBNET

If the upstream network uses the same subnet as the router's LAN (common with
`192.168.88.x`), clients will lose connectivity. Set `LAN_SUBNET` to a different
/24 to avoid the conflict:

```
LAN_SUBNET=192.168.200.0/24
```

The subnet change commands are emitted **last** in the RSC. If importing via SSH
the connection will drop when the bridge IP changes, but `/import` continues
running on the router. Reconnect at the new gateway IP when done.

---

## First-time device setup

**Use Ethernet** (any of ether2–5) rather than WiFi for the initial import.
The RSC disables the default `MikroTik-XXXXXX` WiFi SSID early on, which drops
any wireless session. Ethernet stays up throughout.

On a factory-fresh device, plug into any Ethernet port and SSH or Winbox to
`192.168.88.1` (user `admin`, no password).

Set an admin password immediately:

```
/user set [find name=admin] password="YOUR_ADMIN_PASSWORD"
```

The RSC creates the `root` user automatically. See step 2 of the quickstart
for SSH key installation.

---

## Verification

### WiFi uplink

```
/interface wifi print
```

`wifi1` shows `R` (running) once associated with the upstream AP.

### DHCP lease

```
/ip dhcp-client print
```

`wan-dhcp` shows `bound` with an address from the upstream network.

### WireGuard tunnel

```
/interface wireguard peers print
```

`last-handshake` should be recent (within 30 s given the default keepalive).

### Routing

```
/ip route print where comment=exit-endpoint
/ipv6 route print where dst-address=::/0
```

The endpoint host route and IPv6 default route should both be present and active.

### End-to-end

From a LAN client:

```sh
ping 1.1.1.1
ping6 ipv6.google.com
```

---

## Development

```sh
make build   # cargo build --release
make check   # cargo clippy
make test    # cargo test
```

Requires Nix or a local Rust toolchain. `nix develop` provides cargo and rustc.
