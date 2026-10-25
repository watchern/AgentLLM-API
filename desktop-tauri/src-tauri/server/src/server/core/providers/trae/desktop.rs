//! Trae 桌面端（IDE）登录态读取与解密（「tc」格式）。
//!
//! ── 登录态在哪 ────────────────────────────────────────────
//! Trae CN（VSCode fork）把当前登录态放进
//! `%APPDATA%\Trae CN\User\globalStorage\storage.json` 的
//! `iCubeAuthInfo://icube.cloudide` 键，值是 **base64 的私有加密块**；
//! 同一文件的 `iCubeAuthInfo://icube-dc:<userId>` 存这台设备的密钥对
//! （同一格式加密），`telemetry.machineId` / `telemetry.devDeviceId` 是
//! 出站指纹头要的设备标识（`X-Machine-Id` / `X-Device-Id`，见 headers.rs）。
//!
//! 国际版装在 `%APPDATA%\Trae`，同一位置**明文**存 JSON —— 读取侧兼容
//! 两种形态；解出的 `userRegion.region` 决定谱系（`cn` / `intl`），国际版
//! 凭据由 `add_trae_account` 的谱系闸门按既有文案拒绝（本家未接 Intl 通道，
//! 见 `credentials::INTL_UNSUPPORTED`）。
//!
//! ── 「tc」块怎么解（对照社区逆向实现 ZedeX/trae-local-api 的
//!    CRACK_TUTORIAL，全部常量实测核对过本机 2026-10 的 Trae CN）──────
//! ```text
//! [0..6)    魔数：74 63 05 10 00 00（"tc" AES）或 12 39 20 20 02 03
//!           （AES_PRIVATE，用另一对 salt）
//! [6..38)   32 字节随机数
//! [38..)    AES-128-CBC 密文（PKCS7）
//! ```
//! 密钥派生：`sha512( sha512(random) ‖ (saltA ⊕ saltB) )`，前 16 字节做
//! AES key、接下来 16 字节做 CBC IV；明文 = `sha512(json) ‖ json`（前
//! 64 字节是完整性哈希，解开后校验一次）。
//!
//! 这层加密的目的是**混淆**（salt 硬编码在客户端 JS 里，任何人都能还原），
//! 不是绑定本机 —— 因此网关可以离线解开，不需要 DPAPI 之类的机器密钥。
//!
//! ── 硬约束 ────────────────────────────────────────────────
//! 只读本地文件，没有网络请求；绝不 unwrap/expect（panic=abort）。

use std::path::PathBuf;

use serde_json::Value;

use super::credentials::Credential;
use super::login::CN_DOMAIN;
use super::oauth::DEFAULT_LOGIN_HOST;

/// 64 字节盐 A（客户端 JS 里的硬编码常量，两组按类型各取一对异或）
const SALT_A: [u8; 64] = [
    82, 9, 106, 213, 48, 54, 165, 56, 191, 64, 163, 158, 129, 243, 215, 251, 124, 227, 57, 130,
    155, 47, 255, 135, 52, 142, 67, 68, 196, 222, 233, 203, 84, 123, 148, 50, 166, 194, 35, 61,
    238, 76, 149, 11, 66, 250, 195, 78, 8, 46, 161, 102, 40, 217, 36, 178, 118, 91, 162, 73, 109,
    139, 209, 37,
];
/// 64 字节盐 B（与 SALT_A 配对，用于 "tc" AES 型魔数）
const SALT_B: [u8; 64] = [
    31, 221, 168, 51, 136, 7, 199, 49, 177, 18, 16, 89, 39, 128, 236, 95, 96, 81, 127, 169, 25,
    181, 74, 13, 45, 229, 122, 159, 147, 201, 156, 239, 160, 224, 59, 77, 174, 42, 245, 176, 200,
    235, 187, 60, 131, 83, 153, 97, 23, 43, 4, 126, 186, 119, 214, 38, 225, 105, 20, 99, 85, 33,
    12, 125,
];
/// 64 字节盐 C（AES_PRIVATE 型用的另一对）
const SALT_C: [u8; 64] = [
    191, 192, 216, 250, 122, 246, 220, 97, 31, 254, 98, 27, 8, 72, 71, 176, 135, 99, 96, 18, 127,
    101, 203, 104, 211, 102, 191, 125, 37, 72, 150, 156, 51, 229, 121, 35, 17, 153, 141, 177,
    110, 131, 150, 128, 172, 255, 254, 6, 18, 140, 55, 62, 236, 249, 135, 64, 135, 12, 117, 4,
    89, 149, 168, 209,
];
/// 64 字节盐 D（与 SALT_C 配对）
const SALT_D: [u8; 64] = [
    246, 204, 26, 232, 232, 70, 129, 109, 223, 146, 169, 242, 23, 241, 105, 145, 50, 196, 165,
    42, 254, 120, 3, 54, 244, 207, 209, 85, 53, 6, 138, 106, 175, 148, 31, 204, 186, 186, 165,
    182, 87, 142, 49, 10, 39, 110, 26, 154, 86, 56, 173, 125, 18, 64, 198, 225, 99, 99, 83, 82,
    191, 134, 76, 170,
];

/// 登录态键（当前登录的那个号；`icube-dc:*` 是设备密钥对，按需另取）
const AUTH_KEY: &str = "iCubeAuthInfo://icube.cloudide";

/// 桌面端候选目录（国内版 `Trae CN` 优先，国际版 `Trae` 兜底——后者通常
/// 是明文 JSON，由读取侧兼容；谱系判定交给解出来的 `userRegion`）。
fn storage_files() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        let base = appdata.trim();
        if !base.is_empty() {
            let roaming = PathBuf::from(base);
            dirs.push(roaming.join("Trae CN"));
            dirs.push(roaming.join("Trae"));
        }
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default();
    if !home.as_os_str().is_empty() {
        let fallback = home.join("AppData").join("Roaming");
        dirs.push(fallback.join("Trae CN"));
        dirs.push(fallback.join("Trae"));
    }
    dirs.iter().map(|dir| {
        dir.join("User").join("globalStorage").join("storage.json")
    }).collect()
}

fn sha512(data: &[u8]) -> [u8; 64] {
    use sha2::{Digest, Sha512};
    let mut hasher = Sha512::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// 解一个「tc」加密块，返回明文 JSON 文本。
///
/// 两种魔数都认（AES / AES_PRIVATE，只是选的盐对不同）；明文前 64 字节是
/// SHA-512 完整性哈希，不匹配按损坏报错 —— 静默吞掉会造出一份看似正常的
/// 垃圾凭据。
fn decrypt_tc_blob(blob: &[u8]) -> Result<String, String> {
    if blob.len() < 6 + 32 + 64 + 16 {
        return Err("tc 加密块长度异常".to_string());
    }
    let (salt_a, salt_b) = match &blob[..6] {
        [0x74, 0x63, 0x05, 0x10, 0x00, 0x00] => (&SALT_A, &SALT_B),
        [18, 57, 32, 32, 2, 3] => (&SALT_C, &SALT_D),
        other => return Err(format!("tc 加密块魔数不认识: {other:02x?}")),
    };
    let mut salt = [0u8; 64];
    for (index, byte) in salt.iter_mut().enumerate() {
        *byte = salt_a[index] ^ salt_b[index];
    }
    let random = &blob[6..38];
    let data = &blob[38..];
    let mut combined = [0u8; 128];
    combined[..64].copy_from_slice(&sha512(random));
    combined[64..].copy_from_slice(&salt);
    let final_hash = sha512(&combined);
    let plain = aes128_cbc_decrypt_pkcs7(
        &final_hash[..16],
        &final_hash[16..32],
        data,
    )?;
    if plain.len() < 64 {
        return Err("tc 明文长度异常（不足完整性哈希）".to_string());
    }
    let (stored_hash, body) = plain.split_at(64);
    if stored_hash != sha512(body) {
        return Err("tc 明文完整性校验失败".to_string());
    }
    String::from_utf8(body.to_vec()).map_err(|_| "tc 明文不是合法 UTF-8".to_string())
}

/// AES-128-CBC 解密 + PKCS7 去填充（不引 cbc crate：server 依赖树里的
/// `aes` 0.9 只带 ECB 原语，CBC 的异或链自己写 —— 就是一个逐块
/// `解密块 ⊕ 前一密文块`，不到二十行，省一个 release-candidate 依赖）。
fn aes128_cbc_decrypt_pkcs7(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    use aes::cipher::consts::U16;
    use aes::cipher::{Array, BlockCipherDecrypt, KeyInit};
    const BLOCK: usize = 16;
    if data.is_empty() || data.len() % BLOCK != 0 {
        return Err("AES-CBC 密文长度不是块的整数倍".to_string());
    }
    let cipher =
        aes::Aes128::new_from_slice(key).map_err(|_| "AES-128 密钥长度异常".to_string())?;
    let mut previous = <[u8; BLOCK]>::try_from(iv).map_err(|_| "AES-CBC IV 长度异常".to_string())?;
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(BLOCK) {
        let mut block = <Array<u8, U16>>::try_from(chunk)
            .map_err(|_| "AES-CBC 分块长度异常".to_string())?;
        cipher.decrypt_block(&mut block);
        for index in 0..BLOCK {
            out.push(block[index] ^ previous[index]);
        }
        previous.copy_from_slice(chunk);
    }
    // PKCS7：填充字节数 n ∈ [1,16]，且结尾 n 个字节都等于 n
    let padding = *out.last().ok_or("AES-CBC 解密结果为空")? as usize;
    if padding == 0 || padding > BLOCK || out.len() < padding {
        return Err("PKCS7 填充非法".to_string());
    }
    let body = out.len() - padding;
    if out[body..].iter().any(|byte| *byte as usize != padding) {
        return Err("PKCS7 填充校验失败".to_string());
    }
    out.truncate(body);
    Ok(out)
}

/// 读 storage.json 并解出 `iCubeAuthInfo://icube.cloudide` 的 JSON 对象。
///
/// 返回 `(auth 对象, 所在文件解析出的整个 storage)` —— 后者供调用方顺手取
/// telemetry 机器标识与设备密钥对，避免把文件读两遍。
fn read_local_auth() -> Result<(Value, Value), String> {
    let mut failures: Vec<String> = Vec::new();
    for path in storage_files() {
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                failures.push(format!("读不了 {}（{error}）", path.display()));
                continue;
            }
        };
        let storage: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(error) => {
                failures.push(format!("{} 不是合法 JSON（{error}）", path.display()));
                continue;
            }
        };
        let Some(raw) = storage.get(AUTH_KEY).and_then(Value::as_str) else {
            failures.push(format!("{} 里没有 {}", path.display(), AUTH_KEY));
            continue;
        };
        let trimmed = raw.trim();
        let plain = if trimmed.starts_with('{') {
            // 国际版 / 老版本：明文 JSON
            trimmed.to_string()
        } else {
            let blob = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                trimmed,
            )
            .map_err(|_| "iCubeAuthInfo 不是 base64 也不是明文 JSON".to_string())?;
            decrypt_tc_blob(&blob)?
        };
        let auth: Value = serde_json::from_str(&plain)
            .map_err(|_| "iCubeAuthInfo 明文不是合法 JSON".to_string())?;
        if !auth.is_object() {
            return Err("iCubeAuthInfo 明文根节点不是 JSON 对象".to_string());
        }
        return Ok((auth, storage));
    }
    Err(format!(
        "未找到 Trae 桌面端登录态（{}）。请先在 Trae 客户端登录一次",
        failures.join("；")
    ))
}

/// ISO-8601 时刻（`2026-11-07T02:27:19.642Z`）→ Unix 毫秒；解析不了返回 0
/// （`Credential::expires_at_ms` 对 0 的语义就是「未知」，落盘与续期节奏都认它）。
fn iso_to_epoch_ms(text: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(text.trim())
        .map(|time| time.timestamp_millis())
        .unwrap_or(0)
}

/// 从本机 Trae 客户端读取当前登录态并转成 [`Credential`]。
///
/// 字段口径对照网页登录那条链（`login.rs::complete`）：
///   - `domain` 固定 `trae.cn`、`api_host` 用登录态里的 `host`
///     （没有就回落 `DEFAULT_LOGIN_HOST`）；
///   - `variant` 由登录态的 `userRegion.region` / `account.userTag` 决定
///     （CN → `cn`，其它 → `intl`，国际版由 `add_trae_account` 的闸门拒绝）；
///   - 机器 / 设备标识取 `telemetry.machineId` / `telemetry.devDeviceId`
///     （缺了就空 —— 出站头对空值跳过，见 headers.rs）；
///   - 设备密钥对取 `iCubeAuthInfo://icube-dc:<userId>`（仅当这个键存在；
///     它是「这台设备绑定过哪个账号」的凭据，与登录态同一次加密存储）。
pub fn local_credential() -> Result<Credential, String> {
    let (auth, storage) = read_local_auth()?;
    let text = |key: &str| -> String {
        auth.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_string()
    };
    let access_token = text("token");
    let refresh_token = text("refreshToken");
    if access_token.is_empty() && refresh_token.is_empty() {
        return Err("Trae 桌面端登录态里既没有 token 也没有 refreshToken（可能已退出登录）".to_string());
    }
    let account = auth.get("account").filter(|value| value.is_object());
    let region = auth
        .get("userRegion")
        .and_then(|region| region.get("region"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            account
                .and_then(|account| account.get("userTag"))
                .and_then(Value::as_str)
                .map(str::trim)
        })
        .unwrap_or_default()
        .to_lowercase();
    // 谱系：CN → IDE 谱系 `cn`（ClientID 走 IDE 那把，见 oauth.rs 的表）；
    // 其它（SG / 海外）→ `intl`，让既有闸门用既有文案拒绝。
    let variant = if region.is_empty() || region == "cn" { "cn" } else { "intl" }.to_string();
    let user_id = text("userId");
    let nickname = account
        .and_then(|account| account.get("username"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    // 设备密钥对（icube-dc:<userId>，同一格式加密）
    let mut device_public_key = String::new();
    let mut device_private_key = String::new();
    if !user_id.is_empty() {
        if let Some(raw) = storage
            .get(format!("iCubeAuthInfo://icube-dc:{user_id}"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let plain = if raw.starts_with('{') {
                Some(raw.to_string())
            } else {
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, raw)
                    .ok()
                    .and_then(|blob| decrypt_tc_blob(&blob).ok())
            };
            if let Some(plain) = plain {
                if let Ok(keys) = serde_json::from_str::<Value>(&plain) {
                    device_public_key = keys
                        .get("publicKeyPEM")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    device_private_key = keys
                        .get("privateKeyPEM")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                }
            }
        }
    }
    Ok(Credential {
        access_token,
        refresh_token,
        expires_at: iso_to_epoch_ms(&text("expiredAt")),
        domain: CN_DOMAIN.to_string(),
        api_host: {
            let host = text("host");
            if host.is_empty() { DEFAULT_LOGIN_HOST.to_string() } else { host }
        },
        machine_id: storage
            .get("telemetry.machineId")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_string(),
        device_id: storage
            .get("telemetry.devDeviceId")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_string(),
        variant,
        uid: user_id,
        enterprise_id: String::new(),
        nickname,
        device_public_key,
        device_private_key,
        auth_client_id: String::new(),
    })
}

// ─── 单元测试：加密块格式（自造数据自解，不依赖本机是否装了 Trae）──

#[cfg(test)]
mod tests {
    use super::*;

    /// 用同一套派生流程加密一段明文（测试夹具：能解回来就证明格式实现自洽）
    fn encrypt_tc(body: &str) -> Vec<u8> {
        use aes::cipher::consts::U16;
        use aes::cipher::{Array, BlockCipherEncrypt, KeyInit};
        const BLOCK: usize = 16;
        let mut salt = [0u8; 64];
        for index in 0..64 {
            salt[index] = SALT_A[index] ^ SALT_B[index];
        }
        let random = [7u8; 32];
        let mut combined = [0u8; 128];
        combined[..64].copy_from_slice(&sha512(&random));
        combined[64..].copy_from_slice(&salt);
        let final_hash = sha512(&combined);
        let cipher = aes::Aes128::new_from_slice(&final_hash[..16]).expect("aes key");
        // PKCS7 填充（n ∈ [1,16]，哪怕已经对齐也补一整块）；明文 = 完整性哈希 ‖ 正文
        let mut plain = sha512(body.as_bytes()).to_vec();
        plain.extend_from_slice(body.as_bytes());
        let padding = BLOCK - plain.len() % BLOCK;
        plain.extend(std::iter::repeat(padding as u8).take(padding));
        let mut previous = final_hash[16..32].to_vec();
        let mut encrypted = Vec::with_capacity(plain.len());
        for chunk in plain.chunks_exact(BLOCK) {
            let xored: Vec<u8> = chunk
                .iter()
                .zip(previous.iter())
                .map(|(byte, prev)| byte ^ prev)
                .collect();
            let mut block = <Array<u8, U16>>::try_from(xored.as_slice())
                .expect("异或后的分块长度就是 16");
            cipher.encrypt_block(&mut block);
            // CBC 的下一轮要异或的是**密文块**（不是这里的 xored 中间值）
            previous = block.to_vec();
            encrypted.extend_from_slice(&block);
        }
        let mut blob = vec![0x74, 0x63, 0x05, 0x10, 0x00, 0x00];
        blob.extend_from_slice(&random);
        blob.extend_from_slice(&encrypted);
        blob
    }

    #[test]
    fn tc_blob_round_trip() {
        let body = r#"{"token":"abc","refreshToken":"def","userId":"42"}"#;
        let blob = encrypt_tc(body);
        assert_eq!(body, decrypt_tc_blob(&blob).expect("自加密的块要能解回"));
    }

    #[test]
    fn unknown_magic_is_rejected() {
        assert!(decrypt_tc_blob(&[0u8; 128]).is_err());
    }

    #[test]
    fn iso8601_expiry_becomes_epoch_millis() {
        assert!(iso_to_epoch_ms("2026-11-07T02:27:19.642Z") > 1_790_000_000_000);
        assert_eq!(0, iso_to_epoch_ms("garbage"));
        assert_eq!(0, iso_to_epoch_ms(""));
    }
}
