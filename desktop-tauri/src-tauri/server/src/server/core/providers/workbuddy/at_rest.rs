//! WorkBuddy 5.6.2+ 登录态 AtRestEncryption 的「读端」解密。
//!
//! ── 背景 ──────────────────────────────────────────────────
//! 5.6.2 客户端把 AtRestEncryption 的 buildMode 从 disabled 改成 required，
//! `workbuddy-desktop.info` 里 `auth.accessToken` / `refreshToken`（连
//! `account.nickname` / `phoneNumber`）从明文字符串变成加密信封：
//! `{"$wbEncrypted":1,"envelope":"<base64>"}`。信封（suite=1）是 AES-256-GCM：
//!
//! ```text
//! envelope = {suite:1, keyId(16位hex), nonce(b64,12B), authTag(b64,16B),
//!             ciphertext(b64)}
//! AES key = SHA256(atRestSecretKey 字符串的 UTF-8)   → 32 字节
//! keyId   = SHA256(key).hex 的前 16 位               → 命中校验用
//! AAD     = build_field_aad(keyId)                   → 见下
//! ```
//!
//! AAD（复刻客户端 at-rest-crypto 的 buildAuthenticatedContextAad，
//! scheme=sym-v1、framing=field）：
//! `b"WB-AAD\0"` + suite(1) + 长度前缀"WBEV1" + 长度前缀"sym-v1" +
//! u32(1) + 长度前缀(keyId) + framing 码(2) + 0 + 0，长度前缀是 u32 大端。
//!
//! ── atRestSecretKey 从哪来（多级发现，全部本机只读）──────────
//! 密钥由客户端原生模块运行时提供，**不落盘**、只驻留 WorkBuddy.exe 进程
//! 内存（同一把密钥也包裹 `~/.workbuddy/keyblob` 里的主密钥 —— 但那是
//! 「密钥包主密钥」，方向反着，keyblob 里取不出它自己）。按序尝试：
//!   1. 环境变量 `WORKBUDDY_ATREST_KEY`（44 字符 base64，用户手填）；
//!   2. 环境变量 `WORKBUDDY_ATREST_KEY_FILE`（密钥文件：DPAPI 密文或明文）；
//!   3. 登录态 Data 目录与 `~/.workbuddy` 下的小文件 DPAPI blob 扫描
//!      （个别版本落盘时用得上）；
//!   4. **运行中 WorkBuddy.exe 进程内存扫描**（5.6.2 的主路径）：
//!      先按信封 keyId 的 ASCII / UTF-16LE 命中点在 ±4KB 邻域提取 44 字符
//!      base64 候选；找不到再全内存扫候选。每个候选按「SHA256 派生 keyId
//!      是否等于信封 keyId」严格校验，杜绝误取。
//!      ⚠️ 需要客户端**已启动并登录**、且与本进程同一 Windows 用户（提权
//!      状态不一致时系统会拒绝跨进程读）。
//!
//! 发现到的密钥按 keyId 进程级缓存（导入一次要解 2-3 个字段，不能每个
//! 字段都重扫一遍内存）。密钥材料只在内存中使用，绝不落盘、绝不打日志。
//!
//! 算法与常量对照社区实现（trae-checkin/workbuddy_checkin.py v3.1.x，
//! 其自述已与客户端 `packages/at-rest-crypto` 逐条复刻并对拍）。
//!
//! ── 硬约束 ────────────────────────────────────────────────
//! 只读（文件 / 进程内存都是只读句柄），无网络请求；绝不 unwrap/expect
//! （release 是 panic=abort）。

use std::collections::HashSet;
use std::path::PathBuf;

use serde_json::Value;
use sha2::{Digest, Sha256};

/// atRestSecretKey 的字符串长度（44 字符规范 base64 = 32 字节密钥材料）
const SECRET_KEY_LEN: usize = 44;

/// 进程内存扫描的候选数上限（防长尾：大进程里 base64 形态的字节串很多）
const MAX_MEMORY_CANDIDATES: usize = 200_000;

/// keyId 邻域快速路径的窗口（命中点前后各 4KB）
const NEAR_WINDOW: usize = 4096;

/// 已发现的 at-rest 密钥缓存：keyId(16 hex) → secret(44 字符 base64)。
/// DPAPI / 内存扫描都不便宜，导入一次解多个字段只允许发现一次。
fn discovered_keys() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, String>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

// ─── 派生与校验 ─────────────────────────────────────────────

/// 复刻 normalizeAtRestKeyPayload：key = SHA256(secret 字符串 UTF-8)，
/// keyId = SHA256(key).hex 前 16 位。返回 (key, keyId)。
fn derive_at_rest_key(secret: &str) -> Result<([u8; 32], String), String> {
    if secret.len() != SECRET_KEY_LEN {
        return Err("atRestSecretKey 必须是 44 字符 base64".to_string());
    }
    if !canonical_base64(secret.as_bytes()) {
        return Err("atRestSecretKey 不是规范 base64".to_string());
    }
    let key: [u8; 32] = Sha256::digest(secret.as_bytes()).into();
    let key_id = hex_prefix16(&Sha256::digest(key));
    Ok((key, key_id))
}

/// 32 字节 → 16 hex（keyId 的形态）
fn hex_prefix16(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// 规范 base64 校验：能解码、且重编码与输入一致（Python 侧同一判据）
fn canonical_base64(bytes: &[u8]) -> bool {
    use base64::Engine as _;
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => return false,
    };
    let decoded = match base64::engine::general_purpose::STANDARD.decode(text) {
        Ok(decoded) => decoded,
        Err(_) => return false,
    };
    base64::engine::general_purpose::STANDARD.encode(decoded) == text
}

/// 一个候选（44 字符）是否派生出目标 keyId
fn candidate_matches(candidate: &str, target_key_id: &str) -> bool {
    derive_at_rest_key(candidate)
        .map(|(_, key_id)| key_id == target_key_id)
        .unwrap_or(false)
}

// ─── AAD 与信封 ─────────────────────────────────────────────

/// 复刻 buildAuthenticatedContextAad(keyId, suite=1, framing=field, sym-v1)
fn build_field_aad(key_id: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(64);
    aad.extend_from_slice(b"WB-AAD\x00");
    aad.push(1); // suite
    extend_length_prefixed(&mut aad, b"WBEV1"); // framing=field 的格式 id
    extend_length_prefixed(&mut aad, b"sym-v1"); // 方案
    aad.extend_from_slice(&1u32.to_be_bytes()); // suite 数值
    extend_length_prefixed(&mut aad, key_id.as_bytes()); // 16 hex 字符
    aad.push(2); // framing 码：field
    aad.push(0); // sequence 缺失
    aad.push(0); // final 缺失
    aad
}

/// u32 大端长度前缀 + 内容
fn extend_length_prefixed(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
}

/// 解析后的信封
struct Envelope {
    key_id: String,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    auth_tag: Vec<u8>,
}

/// 解析 suite=1 信封（base64 → JSON → 字段校验，全部照抄客户端的严格度）
fn parse_envelope(envelope_b64: &str) -> Result<Envelope, String> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(envelope_b64.trim())
        .map_err(|_| "envelope 不是合法 base64".to_string())?;
    if base64::engine::general_purpose::STANDARD.encode(&bytes) != envelope_b64.trim() {
        return Err("envelope 不是规范 base64".to_string());
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "envelope 不是合法 JSON".to_string())?;
    let object = value.as_object().ok_or("envelope 必须是对象")?;
    if object.get("suite").and_then(Value::as_i64) != Some(1) {
        return Err("只支持 suite=1 的信封".to_string());
    }
    let key_id = object
        .get("keyId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|text| {
            text.len() == 16
                && text
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or("envelope.keyId 非法")?;
    let decode = |name: &str| -> Result<Vec<u8>, String> {
        let text = object
            .get(name)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("envelope 缺 {name}"))?;
        if text.is_empty() {
            return Ok(Vec::new());
        }
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(text)
            .map_err(|_| format!("envelope.{name} 不是合法 base64"))?;
        if base64::engine::general_purpose::STANDARD.encode(&decoded) != text {
            return Err(format!("envelope.{name} 不是规范 base64"));
        }
        Ok(decoded)
    };
    let nonce = decode("nonce")?;
    let auth_tag = decode("authTag")?;
    let ciphertext = decode("ciphertext")?;
    if nonce.len() != 12 || auth_tag.len() != 16 {
        return Err("nonce(12)/authTag(16) 长度错误".to_string());
    }
    Ok(Envelope {
        key_id,
        nonce,
        ciphertext,
        auth_tag,
    })
}

// ─── 对外：字段解密 ─────────────────────────────────────────

/// 解密一个登录态字段：明文字符串原样返回；`{$wbEncrypted:1, envelope}` 解密。
///
/// 这是导入路径的唯一入口（`account_store::workbuddy_import` 调它）。
pub fn decrypt_field(raw: &Value) -> Result<String, String> {
    if let Value::String(text) = raw {
        return Ok(text.clone()); // 5.5.x 及更早 / 加密关闭时的明文形态
    }
    let Some(object) = raw.as_object() else {
        return Err("字段既不是明文字符串也不是加密信封".to_string());
    };
    if object.get("$wbEncrypted").and_then(Value::as_i64) != Some(1) {
        return Err("字段的 $wbEncrypted 标记不是 1".to_string());
    }
    let envelope = object
        .get("envelope")
        .and_then(Value::as_str)
        .ok_or("加密信封缺少 envelope 字段")?;
    decrypt_envelope(envelope)
}

/// 解密一个信封（base64）为明文 UTF-8 文本
fn decrypt_envelope(envelope_b64: &str) -> Result<String, String> {
    let envelope = parse_envelope(envelope_b64)?;
    let secret = find_at_rest_key(&envelope.key_id)?;
    let (key, _) = derive_at_rest_key(&secret)?;
    let aad = build_field_aad(&envelope.key_id);
    // GCM 的输入是 密文 ‖ tag（Postfix），AAD 参与认证
    let mut joined = envelope.ciphertext.clone();
    joined.extend_from_slice(&envelope.auth_tag);
    let plaintext = aes_gcm_decrypt_raw(&key, &envelope.nonce, &joined, &aad)?;
    String::from_utf8(plaintext).map_err(|_| "解密结果不是合法 UTF-8".to_string())
}

/// AES-256-GCM（密文+tag 拼接、带 AAD）—— 真正的实现
fn aes_gcm_decrypt_raw(
    key: &[u8],
    nonce: &[u8],
    ciphertext_with_tag: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, String> {
    use aes_gcm::aead::consts::U12;
    use aes_gcm::aead::{Aead, Payload};
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
    if key.len() != 32 {
        return Err("AES-256 密钥长度异常".to_string());
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "AES 密钥不受支持".to_string())?;
    let nonce = <Nonce::<U12>>::try_from(nonce).map_err(|_| "GCM nonce 长度异常".to_string())?;
    let payload = Payload {
        msg: ciphertext_with_tag,
        aad,
    };
    cipher
        .decrypt(&nonce, payload)
        .map_err(|_| "AES-GCM 解密失败（密钥或数据不匹配）".to_string())
}

// ─── 密钥发现 ───────────────────────────────────────────────

/// 按多级策略定位 atRestSecretKey（见模块头），返回 44 字符 base64 串。
fn find_at_rest_key(target_key_id: &str) -> Result<String, String> {
    if let Ok(guard) = discovered_keys().lock() {
        if let Some(secret) = guard.get(target_key_id) {
            return Ok(secret.clone());
        }
    }
    let secret = discover_uncached(target_key_id)?;
    if let Ok(mut guard) = discovered_keys().lock() {
        guard.insert(target_key_id.to_string(), secret.clone());
    }
    Ok(secret)
}

/// 未命中缓存时的发现流程（每级失败进下一级；全部失败汇总可操作提示）
fn discover_uncached(target_key_id: &str) -> Result<String, String> {
    // 1) 环境变量：直接给密钥
    if let Ok(secret) = std::env::var("WORKBUDDY_ATREST_KEY") {
        let secret = secret.trim().trim_matches('"').trim_matches('\'').to_string();
        if candidate_matches(&secret, target_key_id) {
            return Ok(secret);
        }
    }
    // 2) 环境变量：密钥文件（DPAPI 密文或明文）
    if let Ok(path) = std::env::var("WORKBUDDY_ATREST_KEY_FILE") {
        let path = path.trim();
        if !path.is_empty() {
            if let Ok(bytes) = std::fs::read(path) {
                if let Some(secret) = candidate_bytes_to_key(&bytes, target_key_id) {
                    return Ok(secret);
                }
            }
        }
    }
    // 3) Data 目录 + ~/.workbuddy 下的 DPAPI blob 扫描（个别落盘情况）
    if let Some(secret) = scan_dpapi_blobs(target_key_id) {
        return Ok(secret);
    }
    // 4) 运行中客户端进程内存扫描（5.6.2 主路径）
    if let Some(secret) = scan_process_memory(target_key_id) {
        return Ok(secret);
    }
    Err(
        "无法定位 WorkBuddy 5.6.2+ 的登录态加密密钥（atRestSecretKey）：已尝试环境变量、\
         DPAPI blob 扫描与运行中客户端进程内存扫描。请确认 WorkBuddy 客户端已启动并登录\
         （密钥只驻留客户端进程内存），且网关与客户端在同一 Windows 用户下运行；\
         也可设置环境变量 WORKBUDDY_ATREST_KEY（44 字符 base64）后重试"
            .to_string(),
    )
}

/// 把一段字节变成候选密钥并校验 keyId：先按 DPAPI 密文解开，解不开按明文串处理
fn candidate_bytes_to_key(bytes: &[u8], target_key_id: &str) -> Option<String> {
    let mut texts: Vec<String> = Vec::new();
    if let Some(plain) = dpapi_unprotect(bytes) {
        texts.push(plain);
    }
    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
        texts.push(text);
    }
    for text in texts {
        let text = text.trim().trim_matches('"').trim_matches('\'').to_string();
        if candidate_matches(&text, target_key_id) {
            return Some(text);
        }
    }
    None
}

/// DPAPI blob 扫描的根目录
fn scan_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for env in ["LOCALAPPDATA", "APPDATA"] {
        if let Ok(base) = std::env::var(env) {
            let base = base.trim();
            if base.is_empty() {
                continue;
            }
            for product in ["CodeBuddyExtension", "WorkBuddyExtension"] {
                roots.push(PathBuf::from(base).join(product).join("Data"));
            }
        }
    }
    if let Some(home) = home_dir() {
        roots.push(home.join(".workbuddy"));
        roots.push(home.join(".config").join("CodeBuddyExtension").join("Data"));
    }
    let mut seen: HashSet<PathBuf> = HashSet::new();
    roots.retain(|root| seen.insert(root.clone()));
    roots
}

fn home_dir() -> Option<PathBuf> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(PathBuf::from)
        .map(|path| {
            if path.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                path
            }
        })
}

/// 扫描小文件里的 DPAPI blob 并逐个校验（Python 侧同款：≤256KB、≤4000 个文件）
fn scan_dpapi_blobs(target_key_id: &str) -> Option<String> {
    const DPAPI_MAGIC: &[u8] = b"\x01\x00\x00\x00\xd0\x8c\x9d\x0a";
    const SKIP_DIRS: [&str; 13] = [
        "cache", "code cache", "gpucache", "file-tree", "extensions",
        "blob_storage", "indexeddb", "service worker", "logs", "crashpad",
        "network", "shadercache", "webrtc",
    ];
    let mut visited = 0usize;
    for root in scan_roots() {
        let mut queue: Vec<PathBuf> = vec![root];
        while let Some(dir) = queue.pop() {
            let entries = match std::fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if path.is_dir() {
                    if !SKIP_DIRS.contains(&name.as_str()) {
                        queue.push(path);
                    }
                    continue;
                }
                visited += 1;
                if visited > 4000 {
                    return None;
                }
                let Ok(meta) = std::fs::metadata(&path) else {
                    continue;
                };
                if meta.len() == 0 || meta.len() > 256 * 1024 {
                    continue;
                }
                let Ok(content) = std::fs::read(&path) else {
                    continue;
                };
                // 文件里出现 DPAPI 魔数就从魔数起（含前面的版本字）整段试
                if let Some(index) = find_subslice(&content, DPAPI_MAGIC) {
                    let start = index.saturating_sub(4); // 版本字（magic 前 4 字节）
                    if let Some(secret) = candidate_bytes_to_key(&content[start..], target_key_id) {
                        return Some(secret);
                    }
                }
            }
        }
    }
    None
}

/// 朴素子串查找（数据量小，不需要更快的算法）
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

// ─── 进程内存扫描（仅 Windows）──────────────────────────────

/// 扫描运行中 WorkBuddy 客户端的进程内存寻找 atRestSecretKey。
/// 非 Windows 或系统调用失败返回 None（调用方进下一级/最终错误）。
fn scan_process_memory(target_key_id: &str) -> Option<String> {
    #[cfg(windows)]
    {
        process_scan::scan(target_key_id)
    }
    #[cfg(not(windows))]
    {
        let _ = target_key_id;
        None
    }
}

#[cfg(windows)]
mod process_scan {
    use super::{candidate_matches, find_subslice, is_base64_char, NEAR_WINDOW, SECRET_KEY_LEN};
    use std::collections::HashSet;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Memory::{
        MEMORY_BASIC_INFORMATION, VirtualQueryEx, MEM_COMMIT, PAGE_GUARD, PAGE_NOACCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    /// 进程名匹配：WorkBuddy.exe（大小写不敏感）；可用环境变量补额外名字
    fn wanted_process_names() -> Vec<String> {
        let mut names = vec!["workbuddy.exe".to_string()];
        if let Ok(extra) = std::env::var("WORKBUDDY_PROCESS_NAMES") {
            for name in extra.split(',') {
                let name = name.trim().to_lowercase();
                if !name.is_empty() && !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        names
    }

    /// 枚举目标进程 PID
    fn list_pids() -> Vec<u32> {
        let mut pids = Vec::new();
        let names = wanted_process_names();
        unsafe {
            let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
                return pids;
            };
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut ok = Process32FirstW(snapshot, &mut entry).is_ok();
            while ok {
                let name = String::from_utf16_lossy(
                    &entry.szExeFile[..entry
                        .szExeFile
                        .iter()
                        .position(|ch| *ch == 0)
                        .unwrap_or(entry.szExeFile.len())],
                )
                .to_lowercase();
                if names.contains(&name) {
                    pids.push(entry.th32ProcessID);
                }
                ok = Process32NextW(snapshot, &mut entry).is_ok();
            }
            let _ = CloseHandle(snapshot);
        }
        pids
    }

    /// 主扫描：Pass 1（keyId 邻域快速路径）→ Pass 2（全内存候选兜底）
    pub fn scan(target_key_id: &str) -> Option<String> {
        for pid in list_pids() {
            unsafe {
                let Ok(handle) = OpenProcess(
                    PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                    false,
                    pid,
                ) else {
                    continue; // 提权不一致 → 打不开，试下一个进程
                };
                let result = scan_handle(&handle, target_key_id);
                let _ = CloseHandle(handle);
                if let Some(secret) = result {
                    return Some(secret);
                }
            }
        }
        None
    }

    /// 单进程扫描：逐段读取已提交可读内存，先定向后兜底
    fn scan_handle(handle: &HANDLE, target_key_id: &str) -> Option<String> {
        let pattern = target_key_id.as_bytes();
        let pattern_utf16: Vec<u8> = target_key_id
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect();
        let mut seen: HashSet<String> = HashSet::new();
        for chunk in readable_regions(handle) {
            // Pass 1：keyId（ASCII）命中点邻域
            let mut hits: Vec<usize> = Vec::new();
            let mut start = 0;
            while let Some(index) = find_subslice(&chunk[start..], pattern) {
                hits.push(start + index);
                start = start + index + 1;
            }
            if !hits.is_empty() {
                for hit in &hits {
                    if let Some(secret) = extract_near(&chunk, *hit, pattern.len(), target_key_id, false, &mut seen) {
                        return Some(secret);
                    }
                }
            }
            // Pass 1：keyId（UTF-16LE）命中点邻域
            let mut hits_utf16: Vec<usize> = Vec::new();
            let mut start = 0;
            while let Some(index) = find_subslice(&chunk[start..], &pattern_utf16) {
                hits_utf16.push(start + index);
                start = start + index + 1;
            }
            if !hits_utf16.is_empty() {
                for hit in &hits_utf16 {
                    if let Some(secret) = extract_near(&chunk, *hit, pattern_utf16.len(), target_key_id, true, &mut seen) {
                        return Some(secret);
                    }
                }
            }
            // Pass 2：全段 base64 候选（ASCII 与 UTF-16LE）
            if let Some(secret) = scan_all_candidates(&chunk, target_key_id, &mut seen) {
                return Some(secret);
            }
        }
        None
    }

    /// 命中点 ±4KB 邻域提取 base64 候选并校验
    fn extract_near(
        chunk: &[u8],
        hit: usize,
        pattern_len: usize,
        target_key_id: &str,
        utf16: bool,
        seen: &mut HashSet<String>,
    ) -> Option<String> {
        let lo = hit.saturating_sub(NEAR_WINDOW);
        let hi = (hit + pattern_len + NEAR_WINDOW).min(chunk.len());
        let segment = &chunk[lo..hi];
        let text: Vec<u8> = if utf16 {
            segment
                .chunks_exact(2)
                .filter(|pair| pair[1] == 0)
                .map(|pair| pair[0])
                .collect()
        } else {
            segment.to_vec()
        };
        candidates_from_bytes(&text, target_key_id, seen)
    }

    /// 一段字节里的全部 44 字符 base64 候选（43 个 base64 字符 + '='）
    fn candidates_from_bytes(
        bytes: &[u8],
        target_key_id: &str,
        seen: &mut HashSet<String>,
    ) -> Option<String> {
        let mut run_start: Option<usize> = None;
        let mut index = 0usize;
        while index < bytes.len() {
            if is_base64_char(bytes[index]) {
                if run_start.is_none() {
                    run_start = Some(index);
                }
                index += 1;
                continue;
            }
            // 非 base64 字符：'=' 收尾时取「前 43 个 base64 字符 + =」
            if bytes[index] == b'=' {
                if let Some(start) = run_start {
                    if index >= start + SECRET_KEY_LEN - 1 {
                        let candidate =
                            String::from_utf8_lossy(&bytes[index + 1 - SECRET_KEY_LEN..=index])
                                .to_string();
                        if !seen.contains(&candidate) {
                            seen.insert(candidate.clone());
                            if seen.len() > super::MAX_MEMORY_CANDIDATES {
                                return None;
                            }
                            if candidate_matches(&candidate, target_key_id) {
                                return Some(candidate);
                            }
                        }
                    }
                }
            }
            run_start = None;
            index += 1;
        }
        None
    }

    /// Pass 2 的全段扫描：ASCII 与 UTF-16LE 两种形态
    fn scan_all_candidates(
        chunk: &[u8],
        target_key_id: &str,
        seen: &mut HashSet<String>,
    ) -> Option<String> {
        if let Some(secret) = candidates_from_bytes(chunk, target_key_id, seen) {
            return Some(secret);
        }
        // UTF-16LE：把宽字符段折成单字节再扫（只在整段都像宽字符时才有意义，
        // 混合段已由 Pass 1 的邻域路径覆盖）
        let narrow: Vec<u8> = chunk
            .chunks_exact(2)
            .filter(|pair| pair[1] == 0)
            .map(|pair| pair[0])
            .collect();
        candidates_from_bytes(&narrow, target_key_id, seen)
    }

    /// 逐段产出目标进程的已提交可读内存（8MB 一块，总量上限 3GB）
    fn readable_regions(handle: &HANDLE) -> Vec<Vec<u8>> {
        const CHUNK: usize = 8 * 1024 * 1024;
        const MAX_TOTAL: usize = 3 * 1024 * 1024 * 1024;
        let mut out = Vec::new();
        let mut total = 0usize;
        let mut address: usize = 0;
        unsafe {
            loop {
                let mut info = MEMORY_BASIC_INFORMATION::default();
                let queried = VirtualQueryEx(
                    *handle,
                    Some(address as *const _),
                    &mut info,
                    std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
                );
                if queried == 0 {
                    break;
                }
                let base = info.BaseAddress as usize;
                let size = info.RegionSize;
                if size == 0 {
                    break;
                }
                let readable = info.State == MEM_COMMIT
                    && info.Protect != PAGE_NOACCESS
                    && (info.Protect & PAGE_GUARD).0 == 0;
                if readable {
                    let mut read = 0usize;
                    while read < size && total < MAX_TOTAL {
                        let want = CHUNK.min(size - read);
                        let mut buffer = vec![0u8; want];
                        let mut got: usize = 0;
                        let ok = ReadProcessMemory(
                            *handle,
                            (base + read) as *const _,
                            buffer.as_mut_ptr() as *mut _,
                            want,
                            Some(&mut got),
                        )
                        .is_ok();
                        if ok && got > 0 {
                            buffer.truncate(got);
                            total += got;
                            out.push(buffer);
                            read += got;
                        } else {
                            break; // 该子段不可读，跳到区域尾
                        }
                    }
                }
                let next = base.saturating_add(size);
                if next <= address {
                    break;
                }
                address = next;
            }
        }
        out
    }
}

/// base64 字母表成员（候选扫描用）
fn is_base64_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/'
}

// ─── DPAPI（Windows）────────────────────────────────────────

/// 解开一段 DPAPI blob（当前用户作用域）；非 Windows / 失败返回 None
fn dpapi_unprotect(blob: &[u8]) -> Option<String> {
    #[cfg(windows)]
    {
        windows_dpapi::unprotect_utf8(blob)
    }
    #[cfg(not(windows))]
    {
        let _ = blob;
        None
    }
}

#[cfg(windows)]
mod windows_dpapi {
    use windows::core::Owned;
    use windows::Win32::Foundation::HLOCAL;
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    /// CryptUnprotectData（flags=0，当前用户）→ UTF-8 文本
    pub fn unprotect_utf8(blob: &[u8]) -> Option<String> {
        if blob.is_empty() {
            return None;
        }
        let mut input = CRYPT_INTEGER_BLOB {
            cbData: blob.len() as u32,
            pbData: blob.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptUnprotectData(&mut input, None, None, None, None, 0, &mut output).ok()?;
            if output.pbData.is_null() || output.cbData == 0 {
                return None;
            }
            let owned = Owned::new(HLOCAL(output.pbData as *mut _));
            let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            drop(owned);
            String::from_utf8(bytes).ok()
        }
    }
}

// ─── 单元测试（不依赖本机装没装 WorkBuddy）──────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 44 字符规范 base64（32 字节密钥材料的真实形态；atRestSecretKey 的长度契约）
    fn fixture_secret() -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode([7u8; 32])
    }

    #[test]
    fn key_derivation_shape() {
        // 44 字符规范 base64（32 字节材料的规范编码形态）
        let secret = fixture_secret();
        assert_eq!(44, secret.len());
        let (key, key_id) = derive_at_rest_key(&secret).expect("44 字符规范 base64 要能派生");
        assert_eq!(32, key.len());
        assert_eq!(16, key_id.len());
        // 同一 secret 派生是确定性的
        let again = derive_at_rest_key(&secret).expect("再派生一次");
        assert_eq!(key_id, again.1);
        // 长度不对 / 非规范形态都要被拒
        assert!(derive_at_rest_key("short").is_err());
        assert!(derive_at_rest_key(&"A".repeat(43)).is_err());
    }

    #[test]
    fn field_aad_shape() {
        let aad = build_field_aad("9127dea1b44020a7");
        // 头 7 字节是域标记
        assert_eq!(&aad[..7], b"WB-AAD\x00");
        // 总长 = 7 + 1 + (4+5) + (4+6) + 4 + (4+16) + 1 + 1 + 1
        assert_eq!(7 + 1 + 9 + 10 + 4 + 20 + 3, aad.len());
        // framing 码（field=2）与结尾两个 0
        assert_eq!(2, aad[aad.len() - 3]);
        assert_eq!(&aad[aad.len() - 2..], &[0u8, 0]);
    }

    #[test]
    fn envelope_rejects_bad_shapes() {
        assert!(parse_envelope("not-base64!!!").is_err());
        // 合法 base64 但不是 JSON
        use base64::Engine as _;
        let json_b64 = base64::engine::general_purpose::STANDARD
            .encode(br#"{"suite":2,"keyId":"9127dea1b44020a7"}"#);
        assert!(parse_envelope(&json_b64).is_err(), "suite=2 要被拒");
    }

    #[test]
    fn candidate_scanner_finds_44_char_runs() {
        let secret = fixture_secret();
        let mut seen = HashSet::new();
        // 密钥埋在一堆普通文本里
        let mut bytes = b"prefix padding ... ".to_vec();
        bytes.extend_from_slice(secret.as_bytes());
        bytes.extend_from_slice(b" ... suffix padding");
        let found = candidates_from_bytes_for_test(&bytes, &mut seen, &secret);
        assert_eq!(Some(secret.clone()), found);
    }

    /// 测试辅助：按 keyId 匹配封装（candidate_matches 需要 keyId，测试里直接
    /// 用派生出的 keyId 反向验证）
    fn candidates_from_bytes_for_test(
        bytes: &[u8],
        seen: &mut HashSet<String>,
        expected: &str,
    ) -> Option<String> {
        let (_, key_id) = derive_at_rest_key(expected).expect("fixture 派生");
        // 直接复用扫描逻辑：跑一遍候选提取并匹配 keyId
        let mut run_start: Option<usize> = None;
        let mut index = 0usize;
        while index < bytes.len() {
            if is_base64_char(bytes[index]) {
                if run_start.is_none() {
                    run_start = Some(index);
                }
                index += 1;
                continue;
            }
            if bytes[index] == b'=' {
                if let Some(start) = run_start {
                    if index >= start + SECRET_KEY_LEN - 1 {
                        let candidate = String::from_utf8_lossy(
                            &bytes[index + 1 - SECRET_KEY_LEN..=index],
                        )
                        .to_string();
                        if !seen.contains(&candidate) {
                            seen.insert(candidate.clone());
                            if candidate_matches(&candidate, &key_id) {
                                return Some(candidate);
                            }
                        }
                    }
                }
            }
            run_start = None;
            index += 1;
        }
        None
    }
}
