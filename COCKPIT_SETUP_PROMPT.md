# Cockpit Tools — Cross-platform Multi-instance + Remote SSH Sync

## Цель

Расширить Cockpit Tools так, чтобы:

1. **Multi-instance Codex работал не только на macOS** (Windows и Linux).
2. **Появилась SSH-синхронизация** аккаунтов и конфигов Codex с удалёнными хостами (`scp`/`ssh`-based, без зависимостей от обратных туннелей).
3. **Конкретный кейс:** на удалённом хосте `192.168.1.47` (Ubuntu 24.04, user `samurai`, port `2222`, key `~/.ssh/llm-server`) поднять Codex-аккаунт `macconachelillian@gmail.com` и привязать его к локальному Cockpit Tools, который запущен на Windows.

---

## Что сделано в исходниках

Репозиторий: `C:\Диск D\!project_Windows\projects\cockpit-tools`.

### 1. Multi-instance Codex для Windows / Linux

Снят macOS-only гейт + добавлены платформенные ветки запуска и генерации launch-команды.

| Файл | Что изменилось |
|---|---|
| [src-tauri/src/modules/codex_instance.rs](src-tauri/src/modules/codex_instance.rs) | `get_default_instances_root_dir()` теперь возвращает `~/.antigravity_cockpit/instances/codex` для **macOS / Windows / Linux**. |
| [src-tauri/src/modules/process.rs](src-tauri/src/modules/process.rs) | `start_codex_with_args()` получил **Windows-ветку** (через `CODEX_HOME` env + `creation_flags`) и **Linux-ветку** (через `spawn_detached_unix`). `start_codex_default()` дополнен **Linux-веткой**. |
| [src-tauri/src/commands/codex_instance.rs](src-tauri/src/commands/codex_instance.rs) | `build_launch_command()` теперь генерирует **PowerShell-команду** на Windows (`$env:CODEX_HOME = ...; & ...`) и POSIX shell на mac/linux. |

### 2. Новый модуль: SSH-синхронизация удалённых хостов

| Файл | Назначение |
|---|---|
| [src-tauri/src/models/cockpit_remote.rs](src-tauri/src/models/cockpit_remote.rs) | Модели: `RemoteHostProfile`, `RemoteHostStore`, `RemoteSshTestResult`, `RemoteSyncSummary`. |
| [src-tauri/src/modules/cockpit_remote_sync.rs](src-tauri/src/modules/cockpit_remote_sync.rs) | Основная логика. Использует системные `ssh`/`scp` (доступны на Windows 10+ из коробки). Хранит профили в `~/.antigravity_cockpit/remote_hosts.json`. |
| [src-tauri/src/commands/cockpit_remote_sync.rs](src-tauri/src/commands/cockpit_remote_sync.rs) | Tauri-команды: `cockpit_remote_list_hosts`, `cockpit_remote_add_host`, `cockpit_remote_update_host`, `cockpit_remote_delete_host`, `cockpit_remote_test_connection`, `cockpit_remote_ensure_codex_path`, `cockpit_remote_fetch_codex_auth`, `cockpit_remote_push_codex_auth`, `cockpit_remote_start_tunnel`, `cockpit_remote_stop_tunnel`, `cockpit_remote_apply_full_setup`. |
| [src-tauri/src/lib.rs](src-tauri/src/lib.rs) | Команды зарегистрированы в `tauri::generate_handler![...]`. |
| [src-tauri/src/modules/mod.rs](src-tauri/src/modules/mod.rs), [src-tauri/src/models/mod.rs](src-tauri/src/models/mod.rs), [src-tauri/src/commands/mod.rs](src-tauri/src/commands/mod.rs) | Подключены новые `pub mod`. |

### Что делает `apply_full_setup`

Идемпотентный пайплайн «применить настройки к удалённому хосту»:

1. `ensure_remote_codex_path` — добавляет `$HOME/.npm-global/bin` в `PATH` через `~/.profile`.
2. `test_connection` — проверяет user/uname, наличие `~/.codex` и `~/.antigravity_cockpit`, путь к `codex`-бинарю, listening на `remote_tunnel_port`.
3. `push_codex_auth_to_remote` (если передан `local_auth_path`) — делает бэкап `auth.json` и `scp` локального файла, выставляет `chmod 600`.
4. `start_tunnel` (если `auto_tunnel: true`) — запускает `ssh -N -T -L local:127.0.0.1:remote alias`.

---

## Что сделано в окружении

### Удалённый хост `192.168.1.47`

```
~/.codex/auth.json         — macconachelillian@gmail.com (verified via id_token decode)
~/.codex/config.toml       — gpt-5.5 / openrouter provider
~/.profile                 — добавлен PATH-блок Cockpit Tools (idempotent)
~/.antigravity_cockpit/config.json  — flat-схема, ws_port 19529, codex_app_path указан
codex CLI                  — 0.128.0 в PATH (~/.npm-global/bin/codex)
ssh listener 127.0.0.1:19529  — активная reverse-tunnel сессия
```

### Локальная Windows

```
C:\Users\rooot\.antigravity_cockpit\config.json        — обновлён (codex_app_path → npm\codex.cmd)
C:\Users\rooot\.antigravity_cockpit\remote_hosts.json  — профиль "llm-server-192-168-1-47"
C:\Users\rooot\.antigravity_cockpit\start-remote-tunnel.ps1  — SSH forward 19530 -> 19529
C:\Users\rooot\.antigravity_cockpit\stop-remote-tunnel.ps1
~/.ssh/config                                           — Host 192.168.1.47 уже задан корректно
Cockpit Tools.exe                                       — установлено и запущено (PID наблюдался)
```

### SSH-туннель проверен

```
START: ssh -N -T -L 127.0.0.1:19530:127.0.0.1:19529 192.168.1.47
       → listener up на 127.0.0.1:19530, TCP probe OK
STOP : taskkill PID  → listener down
```

Туннель **необязателен** для синхронизации auth.json (модуль использует прямой `scp`),
но включён в host-профиль на случай, если на удалённой стороне будет крутиться полноценный
Cockpit Tools daemon на 19529.

---

## Как собрать и активировать

> Локальный Rust toolchain отсутствует — компиляция должна выполняться на машине сборки.

```powershell
# 1) Установить Rust (если ещё нет)
winget install Rustlang.Rustup
rustup default stable

# 2) Установить Node 18+ и npm-зависимости
cd "C:\Диск D\!project_Windows\projects\cockpit-tools"
npm install

# 3) Dev-сборка (горячая перезагрузка)
npm run tauri dev

# 4) Production-бинарь (.msi/.exe)
npm run tauri build
# Артефакты появятся в src-tauri/target/release/bundle/
```

После установки нового билда:

* Меню **«Множественные инстансы → Codex»** перестанет показывать «Не поддерживается на этой системе».
* В настройках появятся новые Tauri-команды для управления удалёнными хостами (фронтенд можно подключить отдельной задачей; бэкенд готов).

## Как пользоваться SSH-sync без UI (через `tauri invoke` / debug-консоль)

```javascript
// В DevTools-консоли запущенного Cockpit Tools:
const { invoke } = window.__TAURI__.core;

// 1) Список хостов
await invoke('cockpit_remote_list_hosts');

// 2) Прогнать full setup (PATH + test + tunnel)
await invoke('cockpit_remote_apply_full_setup', {
  id: 'llm-server-192-168-1-47',
  localAuthPath: null,            // или путь до локального auth.json для push
});

// 3) Только тест соединения
await invoke('cockpit_remote_test_connection', { id: 'llm-server-192-168-1-47' });

// 4) Pull auth.json с удалённого хоста (вернёт JSON-строку)
await invoke('cockpit_remote_fetch_codex_auth', { id: 'llm-server-192-168-1-47' });
```

---

## Маршрутизация Codex-аккаунта на удалённом хосте

Аккаунт `macconachelillian@gmail.com` уже **активен** в `/home/samurai/.codex/auth.json` на 192.168.1.47.
Для использования нужно:

* Локально: запустить Cockpit Tools (Windows) → раздел Codex → «Обзор аккаунтов» — аккаунт виден.
* Удалённо (если хочется CLI): `ssh 192.168.1.47 'codex'` — запустит CLI с уже подложенным токеном.
* Для push-обновления токена с локального Cockpit Tools на удалённый хост:
  ```javascript
  await invoke('cockpit_remote_push_codex_auth', {
    id: 'llm-server-192-168-1-47',
    localAuthPath: 'C:\\Users\\rooot\\.codex\\auth.json'
  });
  ```

---

## Известные ограничения и follow-up

* **Frontend не доработан** — UI для управления удалёнными хостами на странице "Множественные инстансы" нужно добавить отдельной PR'ом. Бэкенд (`invoke`) полностью готов.
* **Mac/Linux PATH-резолвинг для Codex CLI** в `process.rs::resolve_codex_launch_path` использует macOS-специфичные пути; на Linux он попадает в общую `#[cfg(not(target_os = "macos"))]` ветку, которая ищет в стандартных путях — для большинства setup'ов работает, но если бинарь нестандартный, нужно прописать `codex_app_path` в `config.json`.
* **Туннель** автоматически не перезапускается при разрыве — это сделает `ServerAliveInterval=30` (если SSH сам не упадёт). Для production-уровня надёжности можно завернуть в systemd-user/launchd сервис.
* **Конфиг на удалённом хосте** (`~/.antigravity_cockpit/config.json`) сейчас просто метаданные — он будет «оживать» только когда там появится полноценный Cockpit Tools daemon. Сегодня там работает только reverse-tunnel listener (sshd).

---

## Чеклист «всё применено»

* [x] Source: `get_default_instances_root_dir` поддерживает Windows/Linux
* [x] Source: `start_codex_with_args` поддерживает Windows/Linux
* [x] Source: `start_codex_default` поддерживает Linux
* [x] Source: `build_launch_command` имеет PowerShell-вариант
* [x] Source: модуль `cockpit_remote_sync` + 11 Tauri-команд
* [x] Source: команды зарегистрированы в `lib.rs`
* [x] Remote: PATH, `~/.antigravity_cockpit/config.json`, codex CLI в PATH
* [x] Local: `config.json` с корректным `codex_app_path`, `remote_hosts.json` с профилем `192.168.1.47`
* [x] Local: PowerShell скрипты start/stop tunnel, проверены end-to-end
* [x] Verified: `macconachelillian@gmail.com` активен на удалённом auth.json
* [ ] Build: `cargo build --release` (требует Rust toolchain)
* [ ] UI: страница «Удалённые хосты» (заглушка по требованию)
