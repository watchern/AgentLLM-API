//! KukuAI 每日签到：**真实签到（kukuai30）** + 活动任务状态汇总。
//!
//! ── 现行「签到」本体（2026-11-02 实测闭环 + 客户端 1.7.3.107 逆向）──
//! 客户端左下角签到面板（`signinPromo`，连签必得网盘会员月卡）走的是
//! **kukuai30** 体系，与客户端同款接口、无 jsToken 要求：
//! ```text
//! POST /act/v2/kukuai30/homepage?uid=<uk>&tid=291651792210   面板/日历
//! POST /act/v2/kukuai30/signin?uid=<uk>&tid=291651792210     签到动作
//! ```
//! `uid` = 会话三件套的 `uk`。实测网关侧可完整执行：签到后 `signed_today`
//! 翻转、连签天数与下一奖励（网盘会员天卡等）照常推进。已签日重复 POST
//! 回 8001「账号异常」（误导文案，实测语义 = 今日已签）—— 成败以复读
//! homepage 的 `signed_today` 为准。
//!
//! ── 活动任务（次要，积分需客户端完成）──────────────────────
//! `activity_kuku` 的每日任务（与库库AI对话 249 / 浏览知识广场 251）是
//! 另一档奖励，服务端校验**真实行为**（客户端设备指纹 + 页面会话），网关
//! 侧上报（含真实对话 session_id）实测一律 -8 —— `self_report_tasks`
//! 尽力上报并把未达成如实汇总进文案，不伪造成功。
//!
//! ── 历史兼容 ────────────────────────────────────────────────
//! 旧「免费领积分」（freepoint/*）与 `main` / `kuku-svip` 活动均已下线
//! （空 activities / errno=40001），相关入口只剩 `legacy_tasklist` 备查。
//!
//! ── 会话前提 ────────────────────────────────────────────────
//! `with_business_stoken` 换发 genflowpro STOKEN + `ensure_tokens` 的会话
//! 三件套（uk 即签到用的 uid），实测全程 errno=0。
//!
//! ── 硬约束 ──────────────────────────────────────────────────
//! release 是 `panic=abort`：本文件零 unwrap/expect/panic。

use serde_json::{json, Value};

use crate::server::core::account_store::AccountStore;
use crate::server::errors::GatewayError;
use crate::server::logging;

use super::credentials;
use super::http;
use super::BASE_URL;

/// 客户端形态 query（现行活动接口按桌面客户端 1.7.3.107 实测可用；
/// 旧的 web 400/1.4.4 形态在活动接口上已无意义）
const CLIENT_QUERY: &str =
    "app_id=123971023&clienttype=401&version=1.7.3.107&channel=chunlei";

/// 现行每日活动的 task_from（`main` / `kuku-svip` 实测 errno=40001）
const TASK_FROM: &str = "activity_kuku";

/// KukuAI 账号的每日签到（自动签到框架的 claim）。
///
/// 返回 `{success, msg}` —— 与 WorkBuddy / 小浣熊 / AutoClaw / Qoder / Loomy
/// 的 claim 同一形状（`billing::checkin` 的汇总只认这两个字段）。
/// 流程：**真实签到（kukuai30，与客户端左下角面板同款接口）** → 活动任务
/// 状态汇总。签到动作网关可完整执行（uid+tid，无 jsToken 要求，实测
/// signed_today 翻转、连签与「网盘会员天卡」等下一奖励照常推进）；活动
/// 任务（对话 / 浏览）的积分仍需客户端完成，这里如实汇总。
pub async fn claim_daily_checkin(
    store: &AccountStore,
    account_id: &str,
) -> Result<Value, GatewayError> {
    let credentials = credentials::snapshot_for(store, account_id)?;

    // 1) 真实签到（kukuai30：先读面板判已签，未签才发动作，以复读为准）
    let signin = super::tasks::do_signin(&credentials).await?;
    let mut parts: Vec<String> = Vec::new();
    if signin.get("alreadySigned").and_then(Value::as_bool) == Some(true) {
        parts.push("今日已签到".to_string());
    } else if signin.get("signedToday").and_then(Value::as_bool) == Some(true) {
        parts.push("签到成功".to_string());
    } else {
        let message = signin
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("签到未生效");
        return Err(GatewayError::with_status(502, format!("KukuAI 签到失败：{message}")));
    }
    let continuous = signin
        .get("continuousDays")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let next_days = signin
        .get("nextRewardDays")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let next_award = signin
        .get("nextRewardAward")
        .and_then(Value::as_str)
        .unwrap_or("");
    let mut msg = if continuous > 0 {
        format!("{}（连签 {continuous} 天", parts.join("；"))
    } else {
        format!("{}（", parts.join("；"))
    };
    if !next_award.is_empty() && next_days > 0 {
        msg.push_str(&format!("再签 {next_days} 天得 {next_award}）"));
    } else {
        msg.push('）');
    }

    // 2) 活动任务状态汇总（对话 / 浏览需客户端完成，如实带出）
    let task_summary = self_report_tasks(&credentials).await;
    if !task_summary.is_empty() {
        msg.push_str(&format!("；{task_summary}"));
    }

    logging::log("[Checkin]", &format!("✅ {msg}"));
    Ok(json!({ "success": true, "msg": msg }))
}

/// 活动任务（activity_kuku）的可自助任务上报与状态汇总（签到面板的次要
/// 信息；主要奖励在签到本体）。返回空串 = 没有值得带出的状态。
async fn self_report_tasks(credentials: &credentials::KukuCredentials) -> String {
    let tasks = match super::tasks::activity_tasks(credentials).await {
        Ok(tasks) => tasks,
        Err(_) => return String::new(),
    };
    let mut pending: Vec<String> = Vec::new();
    for task in &tasks {
        let task_type = task.get("task_type").and_then(Value::as_str).unwrap_or("");
        if task_type != "249" && task_type != "251" {
            continue; // 邀请类与未知任务不上报
        }
        let name = task
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("任务")
            .to_string();
        let current = task
            .get("current")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let target = task
            .get("target")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if current >= target && target > 0 {
            continue; // 已完成
        }
        // 尽力上报一次（-8 = 服务端校验真实行为未发生，如实在文案里体现）
        let code = task.get("code").and_then(Value::as_str).unwrap_or("");
        if let Ok(report) = super::tasks::report_task(credentials, code).await {
            if report.get("success").and_then(Value::as_bool) == Some(true) {
                continue; // 意外达成的就不进「未达成」清单
            }
        }
        pending.push(if target > 1 {
            format!("{name}（{current}/{target}）")
        } else {
            name
        });
    }
    if pending.is_empty() {
        String::new()
    } else {
        format!("活动任务未达成：{}（需在库库AI客户端完成）", pending.join("、"))
    }
}

/// 旧任务清单入口保留形状（当前 claim 主体已迁到 kukuai30 签到 +
/// self_report_tasks；此函数仅供将来复用时参考 —— 见模块头）。
#[allow(dead_code)]
async fn legacy_tasklist(
    credentials: &credentials::KukuCredentials,
) -> Result<Value, GatewayError> {
    let headers = credentials::request_headers(credentials);
    let list_url = format!(
        "{BASE_URL}/api/taskscore/tasklist?task_from={TASK_FROM}&{CLIENT_QUERY}"
    );
    http::get_json_value(&list_url, &headers, None, Some(30_000)).await
}
