//! 新版 AutoClaw 官方客户端（`AutoClaw-official`）的桌面端登录态读取。
//!
//! ── 为什么要有这个模块 ──────────────────────────────────────
//! AutoClaw 官方客户端改了 userData 目录与登录态的落盘布局：老版把
//! `{deviceId, userInfo, token, refreshToken}`（`enc:` 密文字段）写进
//! `%APPDATA%/AutoClaw/auth.json`；新版（本机实测 2026-10 的 official 构建）
//! **不再写 auth.json**，改为：
//!
//! ```text
//! %APPDATA%/AutoClaw-official/
//!   Local State                                 ← os_crypt 密钥（DPAPI，同老版）
//!   device/
//!     active-product-account.json               ← { accountKey }：当前登录的账号
//!     device-id.json                            ← { deviceId }
//!   accounts/
//!     <accountKey>/
//!       account-credentials.enc                 ← 裸 os_crypt blob（v10 + nonce +
//!       │                                          密文 + tag，无 enc: 前缀/base64）
//!       account-profile.json                    ← { profile: { email, phone, ... } }
//! ```
//!
//! 一个客户端可以登录**多个账号**（`accounts/` 下多个 `<accountKey>` 目录），
//! `device/active-product-account.json` 指明当前生效的那个 —— 「从本机导入」
//! 导入的正是它（与老版「auth.json 只有一份」的语义对齐）。
//!
//! 解密链与老版**完全同源**（见 `crypto.rs` 模块头）：`Local State` 的
//! `os_crypt.encrypted_key` → DPAPI → 32 字节 AES-256 密钥；差别只在密文
//! 形态 —— 老版是 auth.json 里的 `enc:<base64>` **字符串**，这里是**整份
//! 二进制文件**（`crypto::decrypt_os_crypt_blob`）。
//!
//! 明文（实测）是一份 JSON：
//! `{"accessToken": "Bearer eyJ…", "refreshToken": "Bearer eyJ…", "revision",
//!   "schemaVersion", "tokenType", "updatedAt"}` —— 没有 `userInfo` 与
//! `deviceId`：邮箱从 `account-profile.json`（明文）取，设备 id 从
//! `device/device-id.json`（明文）取；`userId` 照旧解 JWT 的 `user_id` 声明。
//!
//! ── 地区仍然由调用方决定 ────────────────────────────────────
//! 新布局同样**没有任何地区标记**（`channel.json` 的 `zai` 是渠道不是地区），
//! 处置与老版 `from_auth_file` 相同：用户在「国内版」区块点导入就是国内版、
//! 在「国际版」区块点就是国际版（见 `credentials::local_credentials` 的说明）。
//!
//! ── 硬约束 ────────────────────────────────────────────────
//! 全是本地文件读 + 解密，没有网络请求；绝不 unwrap/expect（panic=abort）。

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::crypto;

/// 凭证文件大小上限（与老版 `read_json_file` 同一量级：明文 JWT 对加起来
/// 不到 1KB，给足余量同时挡住误指过来的大文件）
const MAX_CREDENTIALS_FILE_SIZE: u64 = 256 * 1024;

/// 一次成功读出的新版登录态（token 已去 `Bearer ` 前缀；不含 JWT 解码 ——
/// 那一步与老版共用 `credentials_from_claims`，在 `credentials.rs` 里做）
pub(crate) struct OfficialLogin {
    /// 凭证文件路径（缓存键的路径指纹来源）
    pub credentials_file: PathBuf,
    /// 凭证文件的 mtime（毫秒；缓存键的时效来源）
    pub modified_at: i64,
    /// access token（已去 Bearer）
    pub token: String,
    /// refresh token（已去 Bearer；该文件形态下始终有值，缺了按空串处理）
    pub refresh_token: String,
    /// `device/device-id.json` 的 deviceId（缺文件为空串，回落 JWT 声明）
    pub device_id: String,
    /// `account-profile.json` 的 `profile.email`（缺文件/无邮箱为空串）
    pub email: String,
}

/// 「该目录下没有可读的新版登录态」的统一错误文案（`credentials_file_in`
/// 的两个调用点共用，保证汇总错误里两种说法不分裂）
pub(crate) fn not_found_message(dir: &Path) -> String {
    format!(
        "未找到 account-credentials.enc（{} 的 accounts 目录里没有可读账号）",
        dir.display()
    )
}

/// 解析某个候选目录下**当前应读**的 `account-credentials.enc` 路径。
///
/// 选择口径：`device/active-product-account.json` 的 `accountKey` 优先；
/// 没有标记（或标记指向的目录不存在/已损坏）时回落「凭证文件 mtime 最新」
/// 的那个目录 —— 多账号并存的客户端里，最近写过登录态的最接近「当前」。
/// 一个可读的候选都没有时返回 None。
pub(crate) fn credentials_file_in(dir: &Path) -> Option<PathBuf> {
    let accounts = dir.join("accounts");
    let mut candidates: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(&accounts)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter_map(|path| {
            let file = path.join("account-credentials.enc");
            let meta = std::fs::symlink_metadata(&file).ok()?;
            // 同老版 read_json_file 的防御：凭证不交给符号链接与异常大小的文件
            if meta.file_type().is_symlink() || !meta.is_file() {
                return None;
            }
            Some((file, meta.modified().ok()?))
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let active = read_json_file(&dir.join("device").join("active-product-account.json"))
        .and_then(|value| {
            value
                .get("accountKey")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .map(str::to_string)
        });
    if let Some(key) = active {
        let wanted = accounts.join(&key).join("account-credentials.enc");
        if let Some((file, _)) = candidates.iter().find(|(file, _)| *file == wanted) {
            return Some(file.clone());
        }
    }
    // 回落：mtime 最新（`SystemTime` 直接比大小即可，无需转毫秒）
    candidates.sort_by_key(|(_, modified)| *modified);
    candidates.pop().map(|(file, _)| file)
}

/// 读取并解密某个候选目录下的新版登录态。
///
/// 任一环节失败返回中文原因（路径信息带上，方便用户对着目录排查）；
/// 目录不存在/没有账号文件也算失败（原因会被 `from_auth_file` 汇总）。
pub(crate) fn read_login(dir: &Path) -> Result<OfficialLogin, String> {
    let label = dir.display();
    let Some(credentials_file) = credentials_file_in(dir) else {
        return Err(not_found_message(dir));
    };
    let meta = std::fs::symlink_metadata(&credentials_file)
        .map_err(|error| format!("无法访问 account-credentials.enc（{label}）: {error}"))?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(format!("account-credentials.enc 路径异常（{label}）"));
    }
    if meta.len() == 0 || meta.len() > MAX_CREDENTIALS_FILE_SIZE {
        return Err(format!("account-credentials.enc 文件大小异常（{label}）"));
    }
    let blob = std::fs::read(&credentials_file)
        .map_err(|error| format!("无法读取 account-credentials.enc（{label}）: {error}"))?;
    // 密钥取同目录的 Local State（os_crypt_aes_key 自带按路径缓存）
    let aes_key = crypto::os_crypt_aes_key(&dir.join("Local State"))?;
    let text = crypto::decrypt_os_crypt_blob(&blob, &aes_key)
        .map_err(|reason| format!("account-credentials.enc 解密失败（{label}）: {reason}"))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|_| format!("account-credentials.enc 明文不是有效 JSON（{label}）"))?;
    let Value::Object(object) = value else {
        return Err(format!("account-credentials.enc 明文根节点不是 JSON 对象（{label}）"));
    };
    // 键名容错与老版口径一致：accessToken / token / access_token 都认
    let field = |keys: &[&str]| -> String {
        keys.iter()
            .find_map(|key| object.get(*key).and_then(Value::as_str))
            .map(crypto::strip_bearer)
            .map(|value| value.trim().to_string())
            .unwrap_or_default()
    };
    let token = field(&["accessToken", "token", "access_token"]);
    let refresh_token = field(&["refreshToken", "refresh_token"]);
    if token.is_empty() && refresh_token.is_empty() {
        return Err(format!("account-credentials.enc 里没有 token（{label}）"));
    }
    let modified_at = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0);
    Ok(OfficialLogin {
        device_id: read_json_file(&dir.join("device").join("device-id.json"))
            .and_then(|value| {
                value
                    .get("deviceId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .map(str::to_string)
            })
            .unwrap_or_default(),
        email: read_json_file(
            &credentials_file
                .parent()
                .unwrap_or(Path::new("."))
                .join("account-profile.json"),
        )
        .and_then(|value| {
            value
                .get("profile")
                .and_then(|profile| profile.get("email"))
                .and_then(Value::as_str)
                .map(str::trim)
                .map(str::to_string)
        })
        .unwrap_or_default(),
        credentials_file,
        modified_at,
        token,
        refresh_token,
    })
}

/// 读一个小 JSON 文件（device-id / active-product-account / account-profile）。
///
/// 这些是**辅助文件**（缺了只影响 deviceId / 邮箱 / 活动账号标记，不影响
/// 凭证本身），因此损坏时静默返回 None 而不是报错 —— 报错只会把真正的
/// 凭证问题淹没在噪音里。凭证文件本体（account-credentials.enc）不走这里，
/// 它有完整的形态防御（见 `read_login`）。
fn read_json_file(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<Value>(&text).ok()
}
