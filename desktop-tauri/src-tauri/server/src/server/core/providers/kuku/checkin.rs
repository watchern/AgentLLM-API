//! KukuAI 每日签到：活动任务面板（taskscore/tasklist）+ 任务上报尝试。
//!
//! ── 上游变迁（2026-11-01 实测 + 客户端 1.7.3.107 app.asar 逆向）──
//! 旧「免费领积分」（freepoint/homenew + freepoint/taskComplete，任务
//! LOGIN/CHAT）已被百度**下线**：homenew 返回空 activities、taskComplete
//! 返回 `complete_status:"FAIL"`——旧实现把它误判成「已领过」，签到中心
//! 从此显示错误状态。
//!
//! 现行体系是**活动任务上报制**（客户端 `/workbench/new-task`）：
//! ```text
//! GET  /api/taskscore/tasklist?task_from=activity_kuku&{客户端形态 query}
//!      → {errno, result:{list:[{task_type, task_id_str, task_name,
//!         task_desc, task_status, task_finish_count, task_mhz, …}]}}
//! POST /api/genflowpro/activity/taskcomplete    （JSON 体）
//!      body: {task_from, task_id_str, app_id, clienttype, version, …}
//!      → {errno, errmsg}
//! ```
//! 实测要点（真实账号多轮验证）：
//!   - `task_from=activity_kuku` 是现行每日活动（`main` / `kuku-svip` 已
//!     返回 errno=40001）；任务：与库库AI对话（249，+10积分×3/日）、
//!     邀请朋友（250）、浏览知识广场（251）；
//!   - **上报必须带服务端认的 `task_id_str`**（清单里拿），缺了/错了都是
//!     errno=-8；带上之后仍 -8 —— 服务端校验**真实行为**（对话要在客户端
//!     会话里发生、浏览要在客户端页面发生，客户端上报时还带设备指纹
//!     devuid/cuid 与 session_id），网关侧代领不被计入（含网关自己发起的
//!     真实对话 + session_id 上报，实测同样 -8）；
//!   - 因此本实现做的是「如实反映 + 尽力上报」：面板展示真实任务与完成
//!     状态，逐任务尝试上报，errno=0 记成功、-8 如实告知需在客户端完成，
//!     其余 errno 透出服务端文案。
//!
//! ── 会话前提 ────────────────────────────────────────────────
//! 与旧实现同一道会话门：调 `with_business_stoken` 先换发 genflowpro
//! STOKEN（`engine.rs`），实测 errno=0 全程可用。
//!
//! ── 硬约束 ──────────────────────────────────────────────────
//! release 是 `panic=abort`：本文件零 unwrap/expect/panic。

use serde_json::{json, Value};

use crate::server::core::account_store::AccountStore;
use crate::server::errors::GatewayError;
use crate::server::logging;

use super::credentials;
use super::http;
use super::{APP_ID, BASE_URL};

/// 客户端形态 query（现行活动接口按桌面客户端 1.7.3.107 实测可用；
/// 旧的 web 400/1.4.4 形态在活动接口上已无意义）
const CLIENT_QUERY: &str =
    "app_id=123971023&clienttype=401&version=1.7.3.107&channel=chunlei";

/// 现行每日活动的 task_from（`main` / `kuku-svip` 实测 errno=40001）
const TASK_FROM: &str = "activity_kuku";

/// 上报后任务状态字段里「未完成」的判定值（清单实测未完成=0；
/// 完成后的取值上游未实测到，按非 0 从宽处理）
const TASK_STATUS_UNFINISHED: i64 = 0;

/// KukuAI 账号的每日签到（自动签到框架的 claim）。
///
/// 返回 `{success, msg}` —— 与 WorkBuddy / 小浣熊 / AutoClaw / Qoder / Loomy
/// 的 claim 同一形状（`billing::checkin` 的汇总只认这两个字段）。
///
/// 流程：任务清单 → 逐任务上报（249 对话 / 251 知识广场这类可自助任务；
/// 250 邀请类跳过）→ 汇总成如实的状态文案。**不再误报**：旧接口的
/// 「FAIL=已领过」判断随 freepoint 一起删除。
pub async fn claim_daily_checkin(
    store: &AccountStore,
    account_id: &str,
) -> Result<Value, GatewayError> {
    let credentials = credentials::snapshot_for(store, account_id)?;
    let prepared = credentials::with_business_stoken(&credentials).await;
    let headers = credentials::request_headers(&prepared);

    // 1) 任务清单（现行活动面板；errno 非 0 = 登录态或活动侧问题）
    let list_url = format!(
        "{BASE_URL}/api/taskscore/tasklist?task_from={TASK_FROM}&{CLIENT_QUERY}"
    );
    let value = http::get_json_value(&list_url, &headers, None, Some(30_000)).await?;
    let errno = value.get("errno").and_then(Value::as_i64).unwrap_or(-1);
    if errno != 0 {
        let message = value
            .get("show_msg")
            .or_else(|| value.get("errmsg"))
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        return Err(GatewayError::with_status(
            401,
            format!("KukuAI 签到失败（tasklist errno={errno}：{message}）——请重新登录该账号"),
        ));
    }
    let tasks = value
        .get("result")
        .and_then(|result| result.get("list"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if tasks.is_empty() {
        let msg = "当前没有进行中的每日活动（任务清单为空）".to_string();
        logging::log("[Checkin]", &format!("✅ {msg}"));
        return Ok(json!({ "success": true, "msg": msg }));
    }

    // 2) 逐任务处理。可自助上报的才尝试（邀请类需要真实受邀人，跳过）；
    //    单个任务失败只记日志继续（与其它家同一取舍）。
    let mut claimed: Vec<String> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let mut done: Vec<String> = Vec::new();
    for task in &tasks {
        let task_type = task
            .get("task_type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let name = task
            .get("task_name")
            .and_then(Value::as_str)
            .unwrap_or("未知任务")
            .to_string();
        let task_id = task
            .get("task_id_str")
            .or_else(|| task.get("task_id"))
            .map(|value| value.to_string())
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();
        let status = task
            .get("task_status")
            .and_then(Value::as_i64)
            .unwrap_or(TASK_STATUS_UNFINISHED);
        let finish_count = task
            .get("task_finish_count")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        // 邀请类任务需要真实受邀人，网关不代报（与旧实现跳过 INVITE_* 同款）
        if task_type == "250" || name.contains("邀请") {
            continue;
        }
        if status != TASK_STATUS_UNFINISHED || finish_count > 0 {
            done.push(name);
            continue;
        }
        if task_id.is_empty() {
            pending.push(name);
            continue;
        }
        // 上报一次（JSON 体，字段按客户端 taskcomplete 的构造口径）
        let url = format!("{BASE_URL}/api/genflowpro/activity/taskcomplete?{CLIENT_QUERY}");
        let body = json!({
            "task_from": TASK_FROM,
            "task_id_str": task_id,
            "app_id": APP_ID,
            "clienttype": 401_i64,
            "version": "1.7.3.107",
        });
        match http::post_json_value(&url, &headers, &body, None, Some(30_000)).await {
            Ok(value) => {
                let report_errno = value.get("errno").and_then(Value::as_i64).unwrap_or(-1);
                if report_errno == 0 {
                    let reward = value
                        .get("data")
                        .and_then(|data| data.get("reward_point"))
                        .and_then(Value::as_i64)
                        .unwrap_or(0);
                    claimed.push(if reward > 0 {
                        format!("{name}+{reward}")
                    } else {
                        name
                    });
                } else {
                    // -8 = 服务端校验「真实行为未发生」（对话/浏览需在客户端
                    // 完成）；其余 errno 原样透出服务端文案
                    let message = value
                        .get("show_msg")
                        .or_else(|| value.get("errmsg"))
                        .and_then(Value::as_str)
                        .unwrap_or("未知错误");
                    logging::log(
                        "[Checkin]",
                        &format!(
                            "⚠️ KukuAI「{name}」上报未达成（errno={report_errno}：{message}）\
                             ——需在库库AI客户端完成对应动作"
                        ),
                    );
                    pending.push(name);
                }
            }
            Err(error) => {
                logging::log(
                    "[Checkin]",
                    &format!("⚠️ KukuAI「{name}」上报失败：{}", error.message),
                );
                pending.push(name);
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    }

    let mut parts: Vec<String> = Vec::new();
    if !claimed.is_empty() {
        parts.push(format!("已上报：{}", claimed.join("、")));
    }
    if !done.is_empty() {
        parts.push(format!("今日已完成：{}", done.join("、")));
    }
    if !pending.is_empty() {
        parts.push(format!(
            "未达成（需在库库AI客户端完成对应动作后由客户端计分）：{}",
            pending.join("、")
        ));
    }
    let msg = if parts.is_empty() {
        "当前没有可自助完成的每日任务".to_string()
    } else {
        parts.join("；")
    };
    logging::log("[Checkin]", &format!("✅ {msg}"));
    Ok(json!({ "success": true, "msg": msg }))
}
