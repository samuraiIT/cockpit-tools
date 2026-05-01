use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use chrono::Utc;
use uuid::Uuid;

use crate::models::cockpit_remote::{
    RemoteHostProfile, RemoteHostStore, RemoteSshTestResult, RemoteSyncStep, RemoteSyncSummary,
};
use crate::modules::{account, atomic_write, logger};

const REMOTE_HOSTS_FILE: &str = "remote_hosts.json";
const SSH_CONNECT_TIMEOUT: u32 = 10;
const SSH_TUNNEL_KEEPALIVE: u32 = 30;
const DEFAULT_REMOTE_CODEX_HOME: &str = "~/.codex";

static REMOTE_STORE_LOCK: std::sync::LazyLock<Mutex<()>> =
    std::sync::LazyLock::new(|| Mutex::new(()));

#[derive(Debug, Clone)]
pub struct CreateRemoteHostParams {
    pub name: String,
    pub host: String,
    pub user: String,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub ssh_alias: Option<String>,
    pub remote_codex_home: Option<String>,
    pub local_tunnel_port: Option<u16>,
    pub remote_tunnel_port: Option<u16>,
    pub auto_tunnel: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateRemoteHostParams {
    pub id: String,
    pub name: Option<String>,
    pub host: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<Option<String>>,
    pub ssh_alias: Option<Option<String>>,
    pub remote_codex_home: Option<Option<String>>,
    pub local_tunnel_port: Option<u16>,
    pub remote_tunnel_port: Option<u16>,
    pub auto_tunnel: Option<bool>,
    pub bound_account_id: Option<Option<String>>,
    pub bound_account_email: Option<Option<String>>,
}

fn store_path() -> Result<PathBuf, String> {
    Ok(account::get_data_dir()?.join(REMOTE_HOSTS_FILE))
}

pub fn load_store() -> Result<RemoteHostStore, String> {
    let path = store_path()?;
    if !path.exists() {
        return Ok(RemoteHostStore::default());
    }
    let bytes = fs::read(&path)
        .map_err(|e| format!("读取远程主机配置失败 ({}): {}", path.display(), e))?;
    if bytes.is_empty() {
        return Ok(RemoteHostStore::default());
    }
    serde_json::from_slice::<RemoteHostStore>(&bytes)
        .map_err(|e| format!("解析远程主机配置失败 ({}): {}", path.display(), e))
}

pub fn save_store(store: &RemoteHostStore) -> Result<(), String> {
    let path = store_path()?;
    let content = serde_json::to_string_pretty(store)
        .map_err(|e| format!("序列化远程主机配置失败: {}", e))?;
    atomic_write::write_string_atomic(&path, &content)
        .map_err(|e| format!("写入远程主机配置失败 ({}): {}", path.display(), e))
}

pub fn list_hosts() -> Result<Vec<RemoteHostProfile>, String> {
    Ok(load_store()?.hosts)
}

pub fn add_host(params: CreateRemoteHostParams) -> Result<RemoteHostProfile, String> {
    let _lock = REMOTE_STORE_LOCK
        .lock()
        .map_err(|_| "无法获取远程配置锁".to_string())?;

    let name = params.name.trim().to_string();
    if name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    let host = params.host.trim().to_string();
    if host.is_empty() {
        return Err("主机地址不能为空".to_string());
    }
    let user = params.user.trim().to_string();
    if user.is_empty() {
        return Err("SSH 用户名不能为空".to_string());
    }

    let mut store = load_store()?;
    if store.hosts.iter().any(|item| item.name == name) {
        return Err(format!("已存在同名远程主机: {}", name));
    }

    let profile = RemoteHostProfile {
        id: Uuid::new_v4().to_string(),
        name,
        host,
        user,
        port: params.port.unwrap_or(22),
        identity_file: params.identity_file.and_then(non_empty),
        ssh_alias: params.ssh_alias.and_then(non_empty),
        remote_codex_home: params.remote_codex_home.and_then(non_empty),
        local_tunnel_port: params.local_tunnel_port.unwrap_or(19528),
        remote_tunnel_port: params.remote_tunnel_port.unwrap_or(19529),
        auto_tunnel: params.auto_tunnel.unwrap_or(true),
        created_at: Utc::now().timestamp_millis(),
        last_synced_at: None,
        last_tunnel_pid: None,
        bound_account_id: None,
        bound_account_email: None,
    };

    store.hosts.push(profile.clone());
    save_store(&store)?;
    Ok(profile)
}

pub fn update_host(params: UpdateRemoteHostParams) -> Result<RemoteHostProfile, String> {
    let _lock = REMOTE_STORE_LOCK
        .lock()
        .map_err(|_| "无法获取远程配置锁".to_string())?;

    let mut store = load_store()?;
    let index = store
        .hosts
        .iter()
        .position(|item| item.id == params.id)
        .ok_or_else(|| "远程主机不存在".to_string())?;
    let host_entry = &mut store.hosts[index];

    if let Some(name) = params.name {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("名称不能为空".to_string());
        }
        host_entry.name = trimmed.to_string();
    }
    if let Some(host) = params.host {
        let trimmed = host.trim();
        if trimmed.is_empty() {
            return Err("主机地址不能为空".to_string());
        }
        host_entry.host = trimmed.to_string();
    }
    if let Some(user) = params.user {
        let trimmed = user.trim();
        if trimmed.is_empty() {
            return Err("SSH 用户名不能为空".to_string());
        }
        host_entry.user = trimmed.to_string();
    }
    if let Some(port) = params.port {
        host_entry.port = port;
    }
    if let Some(identity) = params.identity_file {
        host_entry.identity_file = identity.and_then(non_empty);
    }
    if let Some(alias) = params.ssh_alias {
        host_entry.ssh_alias = alias.and_then(non_empty);
    }
    if let Some(remote_home) = params.remote_codex_home {
        host_entry.remote_codex_home = remote_home.and_then(non_empty);
    }
    if let Some(port) = params.local_tunnel_port {
        host_entry.local_tunnel_port = port;
    }
    if let Some(port) = params.remote_tunnel_port {
        host_entry.remote_tunnel_port = port;
    }
    if let Some(value) = params.auto_tunnel {
        host_entry.auto_tunnel = value;
    }
    if let Some(account_id) = params.bound_account_id {
        host_entry.bound_account_id = account_id.and_then(non_empty);
    }
    if let Some(email) = params.bound_account_email {
        host_entry.bound_account_email = email.and_then(non_empty);
    }

    let updated = host_entry.clone();
    save_store(&store)?;
    Ok(updated)
}

pub fn delete_host(id: &str) -> Result<(), String> {
    let _lock = REMOTE_STORE_LOCK
        .lock()
        .map_err(|_| "无法获取远程配置锁".to_string())?;
    let mut store = load_store()?;
    let index = store
        .hosts
        .iter()
        .position(|item| item.id == id)
        .ok_or_else(|| "远程主机不存在".to_string())?;
    if let Some(pid) = store.hosts[index].last_tunnel_pid {
        let _ = stop_tunnel_pid(pid);
    }
    store.hosts.remove(index);
    save_store(&store)?;
    Ok(())
}

pub fn get_host(id: &str) -> Result<RemoteHostProfile, String> {
    load_store()?
        .hosts
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "远程主机不存在".to_string())
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn build_ssh_target(profile: &RemoteHostProfile) -> String {
    if let Some(alias) = profile.ssh_alias.as_deref() {
        if !alias.trim().is_empty() {
            return alias.trim().to_string();
        }
    }
    format!("{}@{}", profile.user, profile.host)
}

fn build_base_ssh_args(profile: &RemoteHostProfile) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    args.push("-o".to_string());
    args.push(format!("ConnectTimeout={}", SSH_CONNECT_TIMEOUT));
    args.push("-o".to_string());
    args.push("StrictHostKeyChecking=accept-new".to_string());
    args.push("-o".to_string());
    args.push("BatchMode=yes".to_string());

    // Если SSH alias не задан — указываем порт и identity явно
    if profile.ssh_alias.is_none() {
        args.push("-p".to_string());
        args.push(profile.port.to_string());
        if let Some(identity) = profile.identity_file.as_deref() {
            args.push("-i".to_string());
            args.push(identity.to_string());
            args.push("-o".to_string());
            args.push("IdentitiesOnly=yes".to_string());
        }
    }
    args
}

fn run_remote_command(profile: &RemoteHostProfile, remote_cmd: &str) -> Result<String, String> {
    let mut args = build_base_ssh_args(profile);
    args.push(build_ssh_target(profile));
    args.push(remote_cmd.to_string());

    let output = Command::new("ssh")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("启动 ssh 失败: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        return Err(format!(
            "远程命令执行失败 (status={}): {}",
            output.status,
            if stderr.trim().is_empty() {
                stdout.trim()
            } else {
                stderr.trim()
            }
        ));
    }
    Ok(stdout)
}

fn run_scp(profile: &RemoteHostProfile, src: &str, dst: &str) -> Result<(), String> {
    let mut args: Vec<String> = Vec::new();
    args.push("-o".to_string());
    args.push(format!("ConnectTimeout={}", SSH_CONNECT_TIMEOUT));
    args.push("-o".to_string());
    args.push("StrictHostKeyChecking=accept-new".to_string());
    args.push("-o".to_string());
    args.push("BatchMode=yes".to_string());
    if profile.ssh_alias.is_none() {
        args.push("-P".to_string());
        args.push(profile.port.to_string());
        if let Some(identity) = profile.identity_file.as_deref() {
            args.push("-i".to_string());
            args.push(identity.to_string());
            args.push("-o".to_string());
            args.push("IdentitiesOnly=yes".to_string());
        }
    }
    args.push(src.to_string());
    args.push(dst.to_string());

    let output = Command::new("scp")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("启动 scp 失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("scp 失败 (status={}): {}", output.status, stderr.trim()));
    }
    Ok(())
}

pub fn test_connection(id: &str) -> Result<RemoteSshTestResult, String> {
    let profile = get_host(id)?;
    let codex_home = profile
        .remote_codex_home
        .clone()
        .unwrap_or_else(|| DEFAULT_REMOTE_CODEX_HOME.to_string());
    let probe = format!(
        "set -e; echo USER=$(whoami); echo UNAME=$(uname -a); \
         if [ -d {codex_home} ]; then echo CODEX_DIR=present; else echo CODEX_DIR=missing; fi; \
         if [ -d ~/.antigravity_cockpit ]; then echo COCKPIT_DIR=present; else echo COCKPIT_DIR=missing; fi; \
         CODEX_BIN=$(command -v codex 2>/dev/null || true); \
         if [ -z \"$CODEX_BIN\" ] && [ -x ~/.npm-global/bin/codex ]; then CODEX_BIN=~/.npm-global/bin/codex; fi; \
         echo CODEX_BIN=$CODEX_BIN; \
         if ss -tlnp 2>/dev/null | grep -q :{remote_port} ; then echo TUNNEL=listening; else echo TUNNEL=down; fi",
        codex_home = codex_home,
        remote_port = profile.remote_tunnel_port,
    );

    match run_remote_command(&profile, &probe) {
        Ok(stdout) => {
            let mut result = RemoteSshTestResult {
                ok: true,
                message: "连接成功".to_string(),
                remote_user: None,
                remote_uname: None,
                codex_present: false,
                cockpit_dir_present: false,
                codex_binary_path: None,
                remote_tunnel_listening: false,
            };
            for line in stdout.lines() {
                if let Some(rest) = line.strip_prefix("USER=") {
                    result.remote_user = Some(rest.trim().to_string());
                } else if let Some(rest) = line.strip_prefix("UNAME=") {
                    result.remote_uname = Some(rest.trim().to_string());
                } else if line.trim() == "CODEX_DIR=present" {
                    result.codex_present = true;
                } else if line.trim() == "COCKPIT_DIR=present" {
                    result.cockpit_dir_present = true;
                } else if let Some(rest) = line.strip_prefix("CODEX_BIN=") {
                    let trimmed = rest.trim();
                    if !trimmed.is_empty() {
                        result.codex_binary_path = Some(trimmed.to_string());
                    }
                } else if line.trim() == "TUNNEL=listening" {
                    result.remote_tunnel_listening = true;
                }
            }
            Ok(result)
        }
        Err(err) => Ok(RemoteSshTestResult {
            ok: false,
            message: err,
            remote_user: None,
            remote_uname: None,
            codex_present: false,
            cockpit_dir_present: false,
            codex_binary_path: None,
            remote_tunnel_listening: false,
        }),
    }
}

/// Прокладывает Codex в PATH на удалённом хосте через .profile (идемпотентно).
pub fn ensure_remote_codex_path(id: &str) -> Result<String, String> {
    let profile = get_host(id)?;
    let cmd = r#"set -e
PROFILE="$HOME/.profile"
LINE='export PATH="$HOME/.npm-global/bin:$PATH"'
[ -f "$PROFILE" ] || touch "$PROFILE"
if ! grep -Fqx "$LINE" "$PROFILE"; then
    echo "$LINE" >> "$PROFILE"
    echo "PATH_UPDATED"
else
    echo "PATH_OK"
fi
mkdir -p "$HOME/.antigravity_cockpit"
"#;
    run_remote_command(&profile, cmd)
}

/// Чтение auth.json и config.toml с удалённого хоста (Codex CLI).
pub fn fetch_remote_codex_auth(id: &str) -> Result<String, String> {
    let profile = get_host(id)?;
    let codex_home = profile
        .remote_codex_home
        .clone()
        .unwrap_or_else(|| DEFAULT_REMOTE_CODEX_HOME.to_string());
    let cmd = format!(
        "if [ -f {home}/auth.json ]; then cat {home}/auth.json; else echo 'AUTH_NOT_FOUND' >&2; exit 2; fi",
        home = codex_home
    );
    run_remote_command(&profile, &cmd)
}

/// Записывает локальный auth.json (Codex) на удалённый хост (создаёт бэкап существующего).
pub fn push_codex_auth_to_remote(id: &str, local_auth_path: &str) -> Result<String, String> {
    let profile = get_host(id)?;
    let codex_home = profile
        .remote_codex_home
        .clone()
        .unwrap_or_else(|| DEFAULT_REMOTE_CODEX_HOME.to_string());

    let prepare = format!(
        "set -e; mkdir -p {home}; \
         if [ -f {home}/auth.json ]; then cp {home}/auth.json {home}/auth.json.bak-cockpit-$(date +%s); fi; \
         echo OK",
        home = codex_home
    );
    run_remote_command(&profile, &prepare)?;

    let dst = format!("{}:{}/auth.json", build_ssh_target(&profile), codex_home);
    run_scp(&profile, local_auth_path, &dst)?;

    let chmod = format!("chmod 600 {home}/auth.json && echo OK", home = codex_home);
    run_remote_command(&profile, &chmod)?;
    update_last_synced(id)?;
    Ok("auth.json deployed".to_string())
}

fn update_last_synced(id: &str) -> Result<(), String> {
    let _lock = REMOTE_STORE_LOCK
        .lock()
        .map_err(|_| "无法获取远程配置锁".to_string())?;
    let mut store = load_store()?;
    if let Some(host) = store.hosts.iter_mut().find(|item| item.id == id) {
        host.last_synced_at = Some(Utc::now().timestamp_millis());
    }
    save_store(&store)
}

#[cfg(target_os = "windows")]
fn build_tunnel_command(profile: &RemoteHostProfile) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    const DETACHED_PROCESS: u32 = 0x00000008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;

    let mut cmd = Command::new("ssh");
    cmd.arg("-N").arg("-T");
    cmd.arg("-o").arg("ExitOnForwardFailure=yes");
    cmd.arg("-o")
        .arg(format!("ServerAliveInterval={}", SSH_TUNNEL_KEEPALIVE));
    cmd.arg("-o").arg("ServerAliveCountMax=6");
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
    cmd.arg("-o").arg("BatchMode=yes");
    cmd.arg("-L").arg(format!(
        "127.0.0.1:{}:127.0.0.1:{}",
        profile.local_tunnel_port, profile.remote_tunnel_port
    ));

    if profile.ssh_alias.is_none() {
        cmd.arg("-p").arg(profile.port.to_string());
        if let Some(identity) = profile.identity_file.as_deref() {
            cmd.arg("-i").arg(identity);
            cmd.arg("-o").arg("IdentitiesOnly=yes");
        }
    }
    cmd.arg(build_ssh_target(profile));
    cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd
}

#[cfg(not(target_os = "windows"))]
fn build_tunnel_command(profile: &RemoteHostProfile) -> Command {
    let mut cmd = Command::new("ssh");
    cmd.arg("-N").arg("-T");
    cmd.arg("-o").arg("ExitOnForwardFailure=yes");
    cmd.arg("-o")
        .arg(format!("ServerAliveInterval={}", SSH_TUNNEL_KEEPALIVE));
    cmd.arg("-o").arg("ServerAliveCountMax=6");
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
    cmd.arg("-o").arg("BatchMode=yes");
    cmd.arg("-L").arg(format!(
        "127.0.0.1:{}:127.0.0.1:{}",
        profile.local_tunnel_port, profile.remote_tunnel_port
    ));
    if profile.ssh_alias.is_none() {
        cmd.arg("-p").arg(profile.port.to_string());
        if let Some(identity) = profile.identity_file.as_deref() {
            cmd.arg("-i").arg(identity);
            cmd.arg("-o").arg("IdentitiesOnly=yes");
        }
    }
    cmd.arg(build_ssh_target(profile));
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd
}

/// Запускает SSH-туннель в фоне и сохраняет PID в store.
pub fn start_tunnel(id: &str) -> Result<u32, String> {
    let profile = get_host(id)?;

    if let Some(existing_pid) = profile.last_tunnel_pid {
        if is_pid_alive(existing_pid) {
            return Ok(existing_pid);
        }
    }

    let mut cmd = build_tunnel_command(&profile);
    let child = cmd
        .spawn()
        .map_err(|e| format!("启动 SSH 隧道失败: {}", e))?;
    let pid = child.id();
    logger::log_info(&format!(
        "[CockpitRemote] tunnel up host_id={} pid={} L:{}->R:{}",
        profile.id, pid, profile.local_tunnel_port, profile.remote_tunnel_port
    ));
    set_tunnel_pid(&profile.id, Some(pid))?;
    // Даём ssh немного времени, чтобы либо упасть, либо встать
    std::thread::sleep(Duration::from_millis(800));
    if !is_pid_alive(pid) {
        set_tunnel_pid(&profile.id, None)?;
        return Err(
            "SSH 隧道立即退出，请检查 SSH 配置/identity 文件/远程端口"
                .to_string(),
        );
    }
    Ok(pid)
}

/// Останавливает SSH-туннель.
pub fn stop_tunnel(id: &str) -> Result<(), String> {
    let profile = get_host(id)?;
    if let Some(pid) = profile.last_tunnel_pid {
        let _ = stop_tunnel_pid(pid);
    }
    set_tunnel_pid(&profile.id, None)
}

fn set_tunnel_pid(id: &str, pid: Option<u32>) -> Result<(), String> {
    let _lock = REMOTE_STORE_LOCK
        .lock()
        .map_err(|_| "无法获取远程配置锁".to_string())?;
    let mut store = load_store()?;
    if let Some(host) = store.hosts.iter_mut().find(|item| item.id == id) {
        host.last_tunnel_pid = pid;
    }
    save_store(&store)
}

#[cfg(target_os = "windows")]
fn stop_tunnel_pid(pid: u32) -> Result<(), String> {
    let status = Command::new("taskkill")
        .arg("/PID")
        .arg(pid.to_string())
        .arg("/T")
        .arg("/F")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("taskkill 启动失败: {}", e))?;
    if !status.success() {
        return Err(format!("taskkill 退出码: {}", status));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn stop_tunnel_pid(pid: u32) -> Result<(), String> {
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("kill 启动失败: {}", e))?;
    if !status.success() {
        return Err(format!("kill 退出码: {}", status));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn is_pid_alive(pid: u32) -> bool {
    let output = Command::new("tasklist")
        .arg("/FI")
        .arg(format!("PID eq {}", pid))
        .arg("/NH")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout);
            s.contains(&pid.to_string())
        }
        Err(_) => false,
    }
}

#[cfg(not(target_os = "windows"))]
fn is_pid_alive(pid: u32) -> bool {
    let status = Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    matches!(status, Ok(s) if s.success())
}

/// Полный цикл «применить настройки» к удалённому хосту:
///  1) ensure PATH
///  2) test_connection
///  3) push auth.json (если local_auth_path задан)
///  4) start_tunnel (если auto_tunnel)
pub fn apply_full_setup(
    id: &str,
    local_auth_path: Option<String>,
) -> Result<RemoteSyncSummary, String> {
    let mut steps: Vec<RemoteSyncStep> = Vec::new();
    let mut overall_ok = true;

    let path_step = ensure_remote_codex_path(id);
    steps.push(RemoteSyncStep {
        name: "ensure_remote_codex_path".to_string(),
        ok: path_step.is_ok(),
        detail: Some(match &path_step {
            Ok(s) => s.trim().to_string(),
            Err(e) => e.clone(),
        }),
    });
    if path_step.is_err() {
        overall_ok = false;
    }

    let test_step = test_connection(id);
    let test_detail = match &test_step {
        Ok(res) => Some(format!(
            "user={} codex_present={} cockpit_dir={} tunnel_listening={}",
            res.remote_user.clone().unwrap_or_default(),
            res.codex_present,
            res.cockpit_dir_present,
            res.remote_tunnel_listening
        )),
        Err(e) => Some(e.clone()),
    };
    let test_ok = matches!(&test_step, Ok(r) if r.ok);
    steps.push(RemoteSyncStep {
        name: "test_connection".to_string(),
        ok: test_ok,
        detail: test_detail,
    });
    if !test_ok {
        overall_ok = false;
    }

    if let Some(path) = local_auth_path {
        let push_step = push_codex_auth_to_remote(id, &path);
        steps.push(RemoteSyncStep {
            name: "push_codex_auth_to_remote".to_string(),
            ok: push_step.is_ok(),
            detail: Some(match &push_step {
                Ok(s) => s.clone(),
                Err(e) => e.clone(),
            }),
        });
        if push_step.is_err() {
            overall_ok = false;
        }
    }

    let profile = get_host(id)?;
    if profile.auto_tunnel {
        let tunnel_step = start_tunnel(id);
        steps.push(RemoteSyncStep {
            name: "start_tunnel".to_string(),
            ok: tunnel_step.is_ok(),
            detail: Some(match &tunnel_step {
                Ok(pid) => format!("pid={}", pid),
                Err(e) => e.clone(),
            }),
        });
        if tunnel_step.is_err() {
            overall_ok = false;
        }
    }

    Ok(RemoteSyncSummary {
        host_id: id.to_string(),
        steps,
        ok: overall_ok,
    })
}
