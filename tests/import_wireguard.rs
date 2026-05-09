use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

// 32-byte WireGuard keys encoded as base64 (43 data chars + 1 padding)
const VALID_CONFIG: &str = "\
[Interface]
Address = 10.0.0.1/32,fd00::1/128
PrivateKey = AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
MTU = 1320
DNS = 10.0.0.1

[Peer]
PublicKey = BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=
PresharedKey = CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC=
Endpoint = 10.0.0.2:51820
AllowedIPs = ::/0
PersistentKeepalive = 25
";

fn run(input: &str, env_path: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_import-wireguard"))
        .arg(env_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn import-wireguard");
    child.stdin.as_mut().unwrap().write_all(input.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

fn env_path(dir: &TempDir) -> String {
    dir.path().join(".env").to_string_lossy().into_owned()
}

#[test]
fn valid_config_writes_exit_vars() {
    let dir = TempDir::new().unwrap();
    let out = run(VALID_CONFIG, &env_path(&dir));

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let content = std::fs::read_to_string(env_path(&dir)).unwrap();
    assert!(content.contains("EXIT_PRIVATE_KEY=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="));
    assert!(content.contains("EXIT_SERVER_PUBKEY=BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB="));
    assert!(content.contains("EXIT_PRESHARED_KEY=CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC="));
    assert!(content.contains("EXIT_TUNNEL_IPV4=10.0.0.1/32"));
    assert!(content.contains("EXIT_TUNNEL_IPV6=fd00::1/128"));
    assert!(content.contains("EXIT_ENDPOINT_IP=10.0.0.2"));
    assert!(content.contains("EXIT_ENDPOINT_PORT=51820"));
    assert!(content.contains("EXIT_MTU=1320"));
    assert!(content.contains("EXIT_KEEPALIVE=25"));
}

#[test]
fn fresh_env_includes_template_placeholders() {
    let dir = TempDir::new().unwrap();
    let out = run(VALID_CONFIG, &env_path(&dir));

    assert!(out.status.success());

    let content = std::fs::read_to_string(env_path(&dir)).unwrap();
    assert!(content.contains("UPSTREAM_SSID="));
    assert!(content.contains("DEVICE_NAME="));
    assert!(content.contains("NEXTDNS_PROFILE_ID="));
}

#[test]
fn crlf_line_endings_are_stripped() {
    let dir = TempDir::new().unwrap();
    let crlf = VALID_CONFIG.replace('\n', "\r\n");
    let out = run(&crlf, &env_path(&dir));

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let content = std::fs::read_to_string(env_path(&dir)).unwrap();
    assert!(content.contains("EXIT_ENDPOINT_PORT=51820\n"), "trailing CR leaked into value");
}

#[test]
fn existing_env_is_backed_up_and_preserved() {
    let dir = TempDir::new().unwrap();
    std::fs::write(env_path(&dir), "DEVICE_NAME=myrouter\n").unwrap();

    let out = run(VALID_CONFIG, &env_path(&dir));
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    // New .env has EXIT_* and the preserved user value
    let content = std::fs::read_to_string(env_path(&dir)).unwrap();
    assert!(content.contains("EXIT_PRIVATE_KEY="));
    assert!(content.contains("DEVICE_NAME=myrouter"));

    // Exactly one backup file was created
    let backups: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(".env-"))
        .collect();
    assert_eq!(backups.len(), 1);
    let backup_content = std::fs::read_to_string(backups[0].path()).unwrap();
    assert!(backup_content.contains("DEVICE_NAME=myrouter"));
}

#[test]
fn second_run_creates_second_backup() {
    let dir = TempDir::new().unwrap();
    std::fs::write(env_path(&dir), "DEVICE_NAME=first\n").unwrap();
    run(VALID_CONFIG, &env_path(&dir));

    // Overwrite the written .env and run again
    std::fs::write(env_path(&dir), "DEVICE_NAME=second\n").unwrap();
    run(VALID_CONFIG, &env_path(&dir));

    let backups: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(".env-"))
        .collect();
    assert_eq!(backups.len(), 2);
}

#[test]
fn missing_private_key_fails() {
    let config = VALID_CONFIG.replace("PrivateKey = AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\n", "");
    let dir = TempDir::new().unwrap();
    let out = run(&config, &env_path(&dir));

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("PrivateKey"),
        "expected PrivateKey in error, got: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn missing_endpoint_fails() {
    let config = VALID_CONFIG.replace("Endpoint = 10.0.0.2:51820\n", "");
    let dir = TempDir::new().unwrap();
    let out = run(&config, &env_path(&dir));

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("Endpoint"),
        "expected Endpoint in error, got: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn no_ipv6_address_fails() {
    let config = VALID_CONFIG.replace("10.0.0.1/32,fd00::1/128", "10.0.0.1/32");
    let dir = TempDir::new().unwrap();
    let out = run(&config, &env_path(&dir));

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("IPv6"),
        "expected IPv6 in error, got: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
