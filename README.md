# MikroTik hAP ax² Travel Router: Complete Setup Guide

## What This Builds

A travel router based on the MikroTik hAP ax² (C52iG-5HaxD2HaxD) that:

- Connects to an upstream WiFi network (hotel, office, etc.) using the 5 GHz radio as WAN
- Provides a private LAN via the 2.4 GHz radio, wired Ethernet ports (2–5), and an optional
  secondary "travel" SSID on the 5 GHz radio
- NATs LAN clients behind its upstream IP (IPv4)
- Tunnels IPv6 through a WireGuard exit node so LAN clients get IPv6 connectivity without
  any per-device configuration
- Filters DNS through NextDNS over DoH

```
  [Upstream WiFi AP]
         |
    (wifi1, 5GHz)                  ← station-bridge to upstream, WAN interface
         |
   [ MikroTik bridge ]
    /    |    \
  eth2  eth3  wifiTravel(5GHz vAP) ← LAN clients (wired + wireless)
  eth4  eth5
              wifi2(2.4GHz)        ← disabled, not used

  IPv6: all LAN traffic tunnelled through WireGuard exit node (wg exit)
```

---

## Prerequisites

- MikroTik hAP ax² (C52iG-5HaxD2HaxD), factory fresh or reset
- Credentials for the upstream WiFi network you want to connect to
- An IPv6-capable WireGuard exit node (AirVPN or your favorite WireGuard VPN provider) —
  download a WireGuard config for your preferred server
- Winbox (Windows/Mac app) or SSH to reach the router at `192.168.88.1`
- (Optional) NextDNS account and profile ID

---

## 1. Initial Access

A factory-fresh device has no WiFi password. Connect to the `MikroTik-XXXXXX` SSID
or plug into any Ethernet port (2–5). Browse to `http://192.168.88.1` or use Winbox
and connect to `192.168.88.1` with username `admin`, no password.

Open a terminal: **Winbox → New Terminal**, or `ssh admin@192.168.88.1`.

Set an admin password before doing anything else:

```
/user set [find name=admin] password="YOUR_ADMIN_PASSWORD"
```

### Create a root user and install your SSH public key

The default `admin` account is fine for initial setup, but a named `root` account
with full privileges and key-based SSH access is easier to use from scripts and
avoids typing a password every time.

On the MikroTik terminal:

```
/user add name=root group=full
```

Then, from your local machine, push your SSH public key in one command. This works
by writing the key to a temporary file on the router and importing it — no password
prompt after the initial connection:

```sh
ssh -4 \
    -o UserKnownHostsFile=/dev/null \
    -o StrictHostKeyChecking=no \
    root@192.168.88.1 \
    "/file print file=mykey.txt; \
     file set mykey.txt contents=\"$(cat ~/.ssh/id_ed25519.pub)\"; \
     /user ssh-keys import public-key-file=mykey.txt"
```

- `-4` forces IPv4 (avoids ambiguity before IPv6 is configured)
- `UserKnownHostsFile=/dev/null` and `StrictHostKeyChecking=no` skip host key
  verification, which is appropriate here since you are on the LAN talking to a
  freshly reset device
- Adjust `id_ed25519.pub` to match your key file if you use a different key type

After this you can SSH in without a password:

```sh
ssh -4 root@192.168.88.1
```

---

## 2. Create the Bridge Interface

The bridge ties together all LAN-facing interfaces (wired ports + 2.4 GHz AP + travel
vAP) into a single Layer 2 segment.

```
/interface bridge
add name=bridge comment=defconf
```

---

## 3. Configure WiFi Security Profiles

RouterOS 7 uses named security profiles that can be shared across interfaces.

```
/interface wifi security

# Profile for authenticating TO the upstream network (WAN)
add name=Upstream \
    passphrase="YOUR_UPSTREAM_WIFI_PASSWORD" \
    ft=no

# Profile for your local travel AP (LAN)
add name=Travel \
    passphrase="YOUR_TRAVEL_AP_PASSWORD"
```

---

## 4. Configure WiFi Interfaces

### 5 GHz radio — WAN (station-bridge to upstream)

```
/interface wifi
set [find default-name=wifi1] \
    channel.band=5ghz-ax \
    channel.skip-dfs-channels=all \
    configuration.mode=station-bridge \
    configuration.ssid="YOUR_UPSTREAM_SSID" \   ← SSID of the hotel/office/home AP
    security=Upstream \
    security.ft=no \
    security.ft-over-ds=no \
    disabled=no
```

`station-bridge` mode connects the 5 GHz radio as a WiFi client of the upstream AP.
The MikroTik obtains its own IP from the upstream DHCP server (see step 8) and
NATs LAN clients behind it.

### 2.4 GHz radio — disabled

```
/interface wifi
set [find default-name=wifi2] disabled=yes
```

### Secondary travel AP (virtual interface on 5 GHz)

This creates a second SSID on the 5 GHz radio. Useful if you want a separate network
for guests, or a consistently-named SSID you connect your devices to regardless of
what the upstream network is called.

```
/interface wifi
add name=wifiTravel \
    master-interface=wifi1 \
    configuration.mode=ap \
    configuration.ssid="YOUR_TRAVEL_SSID" \     ← e.g. "Amsterdam"
    security=Travel \
    disabled=no
```

---

## 5. Add Ports to the Bridge

The default config puts wifi1 in the bridge; remove it so it stays a standalone WAN
interface. Then add the travel vAP as a LAN member.

```
/interface bridge port
remove [find interface=wifi1]
add bridge=bridge interface=wifiTravel
```

---

## 6. Interface Lists

```
/interface list member
add interface=wifi1  list=WAN
add interface=exit list=WAN
```

---

## 7. LAN IP Address

The default configuration uses `192.168.88.0/24`. If you want a different subnet,
change it here before continuing:

```
/ip address
set [find interface=bridge] address=192.168.X.1/24
```

---

## 8. WAN: DHCP Client

The router obtains its IPv4 address from the upstream WiFi AP.

```
/ip dhcp-client
add interface=wifi1 \
    default-route-tables=main \
    name=wan-dhcp \
    comment=defconf
```

---

## 9. DHCP Server for LAN Clients

The default configuration serves `192.168.88.10–192.168.88.254` on the bridge
interface. If you changed the LAN subnet in step 7, update the pool and network
here to match.

---

## 10. Install CA Certificates

DoH requires the router to verify the TLS certificate of the DNS server. RouterOS
ships without a CA bundle, so fetch and import one before configuring DNS. This
requires basic IPv4 connectivity to already be working (steps 7–9).

```
/tool fetch url=https://curl.se/ca/cacert.pem
/certificate import file-name=cacert.pem
```

---

## 11. DNS

The router acts as a DNS proxy for LAN clients. Here it is configured to use
NextDNS over DoH for privacy and filtering. Replace `YOUR_NEXTDNS_PROFILE_ID` with
your profile ID from nextdns.io, and adjust the upstream resolvers to taste.

```
/ip dns
set allow-remote-requests=yes \
    servers=1.1.1.3,1.0.0.3 \
    use-doh-server="https://dns.nextdns.io/YOUR_NEXTDNS_PROFILE_ID/YOUR_DEVICE_NAME" \
    verify-doh-cert=yes
```

NextDNS resolves via IPv6 as well; pre-seed its addresses as static DNS entries so the
router can reach the DoH server before it has DNS itself:

```
/ip dns static
add name=router.lan address=192.168.88.1 type=A comment=defconf
add name=dns.nextdns.io address=45.90.28.0  type=A
add name=dns.nextdns.io address=45.90.30.0  type=A
add name=dns.nextdns.io address=2a07:a8c0:: type=AAAA
add name=dns.nextdns.io address=2a07:a8c1:: type=AAAA
```

---

## 12. NAT (IPv4)

Masquerade all LAN traffic behind the router's upstream IP.

```
/ip firewall nat
add action=masquerade chain=srcnat \
    ipsec-policy=out,none \
    out-interface-list=WAN \
    comment="defconf: masquerade"
```

---

## 13. Firewall (IPv4)

The factory default rules are sufficient. They provide stateful filtering on both
the input and forward chains, accepting established/related traffic and dropping
anything arriving from WAN that wasn't initiated from the LAN side. Adjust if you
need to expose services or tighten restrictions beyond the defaults.

---

## 14. Firewall (IPv6)

Same as IPv4 — the factory defaults are sufficient. They block known-bad address
ranges, accept ICMPv6 and established/related traffic, and drop unsolicited inbound
connections from the WAN side. Adjust as needed.

---

## 15. IPv6 via WireGuard Exit Node

The upstream WiFi network provides IPv4 only. We tunnel IPv6 through a WireGuard exit
node (AirVPN or your favorite WireGuard VPN provider). Because the exit node assigns a
single /128 (not a prefix), LAN clients get ULA addresses from a locally assigned /64,
masqueraded behind the tunnel address via NAT66 — the IPv6 equivalent of the IPv4
masquerade already in place.

Rather than embedding secrets in these instructions, a pair of bash scripts in this
repo build the RouterOS setup script from a `.env` file.

### Setup

Copy the env template and fill in the network values:

```sh
cp .env.example .env
$EDITOR .env
```

Populate the `EXIT_*` values by piping your downloaded WireGuard config through the
parser. This overwrites only the `EXIT_*` keys, leaving everything else intact:

```sh
nix run .#import-wireguard < wireguard.conf
```

Generate the RouterOS script:

```sh
nix run .#generate > setup.rsc
```

Apply it on the router:

```sh
ssh root@192.168.88.1 "/import file-name=setup.rsc"
```

### What LAN clients see

- Each device auto-configures a ULA IPv6 address via SLAAC (no manual setup)
- IPv6 default gateway is the router
- DNS is served by the MikroTik (NextDNS DoH handles resolution)
- Outbound IPv6 is masqueraded behind the exit node's tunnel address
- IPv4 is unaffected — still routes directly through wifi1

---

## Verification

### WiFi uplink

```
/interface wifi print
```

`wifi1` should show `R` (running) in the flags column once it associates with the
upstream AP.

### IPv4 connectivity

```
/ip dhcp-client print
```

`wan-dhcp` should show `bound` with an address from the upstream network.

```
/ping 1.1.1.1
```

### WireGuard exit tunnel

```
/interface wireguard peers print
```

The `last-handshake` field should be non-zero and recent (within 30 s given
`persistent-keepalive=15s`).

```
/ipv6 route print
```

The `::/0` route should show `reachable` via `exit`.

### IPv6 from a LAN client

```
ping6 ipv6.google.com
```

