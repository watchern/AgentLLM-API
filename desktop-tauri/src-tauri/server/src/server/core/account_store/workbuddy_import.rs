//! WorkBuddy 官方桌面客户端登录态导入。
//!
//! 官方客户端（Tencent WorkBuddy / WorkBuddy AI）登录后把会话凭证落在本机
//! （官方 FAQ 的排障步骤里就让你列这个目录）：
//!   Windows：`%LOCALAPPDATA%\CodeBuddyExtension\Data\Public\auth\`
//!   macOS：  `~/Library/Application Support/CodeBuddyExtension/Data/Public/auth/`
//! 明文形态是嵌套 JSON `{auth:{accessToken, refreshToken, expiresAt(毫秒), …},
//! account:{uid, nickname, …}}`，与 `store_crud::add_account` 的嵌套形同构 ——
//! 因此本文件只做「定位文件 + 读出来 + 必要时解密」，落库全部复用 `add_account`
//! （撞 id 保护、edition 解析、token 长度校验都在那边）。
//!
//! ── 2026-10 起的新版客户端（5.6.2+）：token 字段被整段加密 ────
//! 新版把 `auth.accessToken` / `auth.refreshToken`（连 `account.nickname` /
//! `phoneNumber` 一起）从明文字符串换成了**加密信封**
//! `{"$wbEncrypted":1,"envelope":"…"}`（AES-256-GCM，密钥发现与解密链见
//! `providers::workbuddy::at_rest` —— 环境变量 / DPAPI blob 扫描 /
//! 运行中客户端进程内存扫描，主路径要求 WorkBuddy 客户端正在运行）。
//! 导入策略：按「当前文件 → 历史备份（新→旧）」逐个尝试，对每个候选先
//! 解密信封字段再交给 `add_account`；解密失败（客户端没开 / 提权不一致）
//! 时继续试下一份（备份可能还是明文形态），全部失败才报可操作的错误。
//!
//! ── 两家两个文件前缀（按 provider 参数化，与 AutoClaw 的地区同款）──
//!   `workbuddy`（国内版）      → `workbuddy-desktop.info`（实测 domain
//!                                `www.workbuddy.cn`）
//!   `workbuddy-intl`（国际版） → `workbuddy-desktop-ai.info`（实测 domain
//!                                `www.workbuddy.ai`，产品名 WorkBuddy AI）
//!
//! ── 历次登录的残留副本 ────────────────────────────────────
//! 每次重新登录官方客户端会把上一份改名成
//! `workbuddy-desktop[-ai].<UTC时间戳>.<pid>.<uuid>.info` 留在原目录**不删**。
//! 备份里的 accessToken 可能已过期，refreshToken 一般仍在有效期内，
//! 落库后由刷新链路续期。
//!
//! ── 硬约束 ────────────────────────────────────────────────
//! 只读文件 + 落库，没有网络请求；错误一律 `AccountStoreError`（400 带可操作
//! 文案），不 panic（release 是 panic=abort）。

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::server::core::account_store::store::{AccountStore, AccountStoreError};
use crate::server::core::providers::workbuddy::at_rest;
use crate::server::core::providers::{kind_from_id, kind_id, ProviderKind};
use crate::server::logging;

/// 官方客户端凭证目录（Windows）：`%LOCALAPPDATA%\CodeBuddyExtension\…`
fn auth_directory_windows() -> Option<PathBuf> {
    std::env::var("LOCALAPPDATA")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|base| {
            PathBuf::from(base)
                .join("CodeBuddyExtension")
                .join("Data")
                .join("Public")
                .join("auth")
        })
}

/// 官方客户端凭证目录（macOS）：`~/Library/Application Support/CodeBuddyExtension/…`
/// （路径取自官方 FAQ 排障步骤的 `ls` 命令，逐字一致）
fn auth_directory_macos() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .filter(|value| !value.trim().is_empty())?;
    Some(
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("CodeBuddyExtension")
            .join("Data")
            .join("Public")
            .join("auth"),
    )
}

/// 当前（不带时间戳）登录态文件的文件名。
fn current_file_name(intl: bool) -> &'static str {
    if intl {
        "workbuddy-desktop-ai.info"
    } else {
        "workbuddy-desktop.info"
    }
}

/// 该目录下本家候选文件的前缀（含结尾的点，天然把另一家排除在外：
/// `workbuddy-desktop.` 不匹配 `workbuddy-desktop-ai.2026-…`，反之亦然）。
fn backup_prefix(intl: bool) -> &'static str {
    if intl {
        "workbuddy-desktop-ai."
    } else {
        "workbuddy-desktop."
    }
}

/// 本家候选文件清单：当前文件在前，历史备份按文件名时间戳**新→旧**排后。
/// 空清单 = 目录里没有任何本家文件。
fn candidate_auth_files(dir: &Path, intl: bool) -> Vec<PathBuf> {
    let mut candidates = vec![dir.join(current_file_name(intl))];
    let prefix = backup_prefix(intl);
    let mut backups: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(prefix) && name.ends_with(".info"))
        })
        .collect();
    // 时间戳在文件名里（ISO 形态、零填充），字典序就是时间序；新→旧
    backups.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    candidates.extend(backups);
    candidates
        .into_iter()
        .filter(|path| path.is_file())
        .collect()
}

/// 读并解析一个 `.info` 文件（JSON 对象）。
fn read_info_payload(path: &Path) -> Result<Value, AccountStoreError> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        AccountStoreError::bad_request(format!(
            "读取登录态文件失败（{}）：{error}",
            path.display()
        ))
    })?;
    serde_json::from_str(&text).map_err(|error| {
        AccountStoreError::bad_request(format!(
            "登录态文件不是合法 JSON（{}）：{error}",
            path.display()
        ))
    })
}

/// payload 里能否读到**明文**的 accessToken（嵌套 `auth.accessToken` 与顶层都认，
/// 与 `store_crud::add_account` 的取值口径一致）。
fn has_plaintext_access_token(payload: &Value) -> bool {
    let auth = payload.get("auth");
    ["accessToken", "token", "access_token"]
        .iter()
        .find_map(|key| auth.and_then(|auth| auth.get(*key)))
        .or_else(|| {
            ["accessToken", "token", "access_token"]
                .iter()
                .find_map(|key| payload.get(*key))
        })
        .and_then(Value::as_str)
        .is_some_and(|text| !text.trim().is_empty())
}

/// 解密 payload 里的加密信封字段（5.6.2+），返回可直接落库的明文形态。
///
/// - `auth.accessToken` / `auth.refreshToken`（含顶层平铺形态）：**严格** ——
///   解不开这个候选就失败（调用方继续试下一份文件）；
/// - `account.nickname`：**尽力** —— 它只是展示名，解不开不挡导入。
fn decrypt_payload_fields(payload: &Value) -> Result<Value, String> {
    let mut prepared = payload.clone();
    let Some(object) = prepared.as_object_mut() else {
        return Err("登录态根节点不是 JSON 对象".to_string());
    };
    // auth 段
    if let Some(auth) = object.get_mut("auth").and_then(Value::as_object_mut) {
        for key in ["accessToken", "token", "access_token", "refreshToken", "refresh_token"] {
            if let Some(value) = auth.get_mut(key) {
                if value.is_string() {
                    continue; // 明文（5.5.x / 加密关闭）
                }
                let decrypted = at_rest::decrypt_field(value)?;
                auth.insert(key.to_string(), Value::String(decrypted));
            }
        }
    }
    // 顶层平铺形态（wb2api 手写形）
    for key in ["accessToken", "token", "access_token", "refreshToken", "refresh_token"] {
        if let Some(value) = object.get_mut(key) {
            if value.is_string() {
                continue;
            }
            let decrypted = at_rest::decrypt_field(value)?;
            object.insert(key.to_string(), Value::String(decrypted));
        }
    }
    // 昵称：尽力解密（失败保留原样，只影响展示名）
    if let Some(nickname) = object
        .get_mut("account")
        .and_then(Value::as_object_mut)
        .and_then(|account| account.get_mut("nickname"))
    {
        if !nickname.is_string() {
            if let Ok(decrypted) = at_rest::decrypt_field(nickname) {
                *nickname = Value::String(decrypted);
            }
        }
    }
    Ok(prepared)
}

impl AccountStore {
    /// 从本机官方客户端的登录态文件导入一个 WorkBuddy 账号。
    ///
    /// `provider` 必须是 `workbuddy` / `workbuddy-intl`（决定读哪一组文件、账号
    /// 落进哪一家）；`name` 是用户填的备注名，缺省用文件里的昵称。落库复用
    /// [`AccountStore::add_account`]：解密后的明文形态原样传入，缺 uid 等
    /// 校验仍由它报 400。
    ///
    /// 候选文件按「当前 → 备份（新→旧）」逐个试：明文形态（5.5.x）直接用；
    /// 加密信封（5.6.2+）先解密（密钥发现见 `providers::workbuddy::at_rest`，
    /// **需要 WorkBuddy 客户端正在运行**）——解密失败就试下一份，全部失败时
    /// 把各份的原因汇总成一条可操作的错误。
    pub fn import_workbuddy_desktop_account(
        &self,
        provider: &str,
        name: Option<&str>,
    ) -> Result<Value, AccountStoreError> {
        let intl = match kind_from_id(provider) {
            Some(ProviderKind::WorkBuddy) => false,
            Some(ProviderKind::WorkBuddyIntl) => true,
            _ => {
                return Err(AccountStoreError::bad_request(format!(
                    "导入桌面端登录态只支持 WorkBuddy 国内版 / 国际版，不支持：{provider}"
                )))
            }
        };
        let dir = auth_directory_windows()
            .or_else(auth_directory_macos)
            .ok_or_else(|| {
                AccountStoreError::bad_request(
                    "无法定位用户目录，找不到官方客户端的登录态文件",
                )
            })?;
        let candidates = candidate_auth_files(&dir, intl);
        if candidates.is_empty() {
            return Err(AccountStoreError::bad_request(format!(
                "未在官方客户端目录找到登录态文件（{}）：请先在官方 Tencent WorkBuddy 客户端里登录一次",
                dir.display()
            )));
        }
        let mut first_payload: Option<(Value, PathBuf)> = None;
        let mut failures: Vec<String> = Vec::new();
        for path in &candidates {
            let payload = read_info_payload(path)?;
            if first_payload.is_none() {
                first_payload = Some((payload.clone(), path.clone()));
            }
            // 5.6.2+ 加密信封 → 解密（密钥发现见 providers::workbuddy::at_rest）；
            // 明文形态原样通过。解密失败（客户端没开等）→ 试下一份备份。
            let prepared = match decrypt_payload_fields(&payload) {
                Ok(prepared) => prepared,
                Err(reason) => {
                    failures.push(format!("{}：{}", path.display(), reason));
                    continue;
                }
            };
            if has_plaintext_access_token(&prepared) {
                let account = self.add_account(&prepared, name, Some(kind_id(if intl {
                    ProviderKind::WorkBuddyIntl
                } else {
                    ProviderKind::WorkBuddy
                })));
                if account.is_ok() {
                    logging::log(
                        "[Accounts]",
                        &format!(
                            "📥 已从官方客户端导入 {}账号（{}）",
                            if intl { "国际版" } else { "国内版" },
                            path.display()
                        ),
                    );
                }
                return account;
            }
            failures.push(format!("{}：文件里没有可用的 accessToken", path.display()));
        }
        // 走到这里 = 每一份候选都读不出可用的明文 token。把各份的失败原因
        // 汇总给出（最常见的两条：客户端未运行导致密钥定位失败；确实没登录）。
        let _ = first_payload;
        let joined = failures.join("；");
        Err(AccountStoreError::bad_request(if joined.is_empty() {
            "未在官方客户端目录找到可导入的登录态：请先在 WorkBuddy 客户端里登录一次".to_string()
        } else {
            format!("导入 WorkBuddy 桌面端登录态失败：{joined}")
        }))
    }
}
