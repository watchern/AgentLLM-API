//! KukuAI 活动任务（「成长任务」形态的数据源）。
//!
//! ── 上游（2026-11-01 客户端 1.7.3.107 逆向 + 真实账号实测）──────
//! ```text
//! GET  /api/taskscore/tasklist?task_from=activity_kuku&{客户端形态 query}
//!      → {errno, result:{list:[{task_type, task_id_str, task_name,
//!         task_desc, task_status, task_finish_count, task_count_max, …}]}}
//! POST /api/genflowpro/activity/taskcomplete    （JSON 体）
//!      body: {task_from, task_id_str, app_id, clienttype, version, …}
//!      → {errno, errmsg, data?}
//! ```
//! 现行每日活动是 `task_from=activity_kuku`（`main` / `kuku-svip` 实测
//! errno=40001）。任务：与库库AI对话（249）、邀请朋友（250）、
//! 浏览知识广场（251）。
//!
//! ── 为什么只读 + 尽力上报，不承诺计分 ────────────────────────
//! 服务端校验**真实行为**：实测带正确 task_id_str、真实对话 session_id、
//! 设备标识上报一律 errno=-8（客户端上报时还带自家设备指纹与会话上下文，
//! 网关侧代发不被计入）。所以这里的上报是 best-effort：errno=0 照常计成功，
//! -8 如实返回「需在库库AI客户端完成」；展示面板（任务清单 + 完成状态）
//! 则完全真实可用。
//!
//! ── 硬约束 ────────────────────────────────────────────────
//! 只读 + 上报两类请求，绝不 unwrap/expect（release 是 panic=abort）。

use serde_json::{json, Value};

use crate::server::errors::GatewayError;

use super::credentials;
use super::http;
use super::{APP_ID, BASE_URL};

/// 客户端形态 query（与 checkin.rs 同一口径）
const CLIENT_QUERY: &str =
    "app_id=123971023&clienttype=401&version=1.7.3.107&channel=chunlei";

/// 现行每日活动的 task_from
const TASK_FROM: &str = "activity_kuku";

/// 可自助上报的任务类型（249=与库库AI对话、251=浏览知识广场）；
/// 250=邀请朋友需要真实受邀人，不代报
fn self_serve(task_type: &str) -> bool {
    task_type == "249" || task_type == "251"
}

/// 拉取现行每日活动的任务清单，映射成**成长任务行**的展示形状
/// （`api::growth` 的 `task_row` 同款字段：code/title/description/status/
/// current/target/supported/unforgeable…，前端 GrowthRow 直接渲染）。
pub async fn activity_tasks(
    credentials: &credentials::KukuCredentials,
) -> Result<Vec<Value>, GatewayError> {
    let prepared = credentials::with_business_stoken(credentials).await;
    let headers = credentials::request_headers(&prepared);
    let url = format!(
        "{BASE_URL}/api/taskscore/tasklist?task_from={TASK_FROM}&{CLIENT_QUERY}"
    );
    let value = http::get_json_value(&url, &headers, None, Some(30_000)).await?;
    let errno = value.get("errno").and_then(Value::as_i64).unwrap_or(-1);
    if errno != 0 {
        let message = value
            .get("show_msg")
            .or_else(|| value.get("errmsg"))
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        return Err(GatewayError::with_status(
            401,
            format!("KukuAI 活动任务清单读取失败（errno={errno}：{message}）——请重新登录该账号"),
        ));
    }
    let tasks = value
        .get("result")
        .and_then(|result| result.get("list"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(tasks.iter().map(task_row).collect())
}

/// 一条任务 → 成长任务展示行。
///
/// `status` 复用 WorkBuddy 成长任务的口径词汇：已完成（今日次数用尽 /
/// task_status 非 0）→ `claimed`（界面 ✓已领取），未完成 → `accepted`。
/// `unforgeable` 如实标注：这些任务必须在库库AI客户端里完成真实动作，
/// 网关代上报不计分（界面显示「需真实操作」徽标 + 原因）。
fn task_row(task: &Value) -> Value {
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
    let description = task
        .get("task_desc")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let code = task
        .get("task_id_str")
        .or_else(|| task.get("task_id"))
        .map(|value| value.to_string())
        .unwrap_or_default()
        .trim_matches('"')
        .to_string();
    let finish_count = task
        .get("task_finish_count")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let count_max = task
        .get("task_count_max")
        .and_then(Value::as_i64)
        .unwrap_or(1);
    let status_raw = task
        .get("task_status")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let finished = status_raw != 0 || (count_max > 0 && finish_count >= count_max);
    json!({
        "code": code,
        "title": name,
        "description": description,
        "status": if finished { "claimed" } else { "accepted" },
        "current": finish_count,
        "target": count_max,
        "supported": self_serve(&task_type),
        // 面板行如实标注：这类任务网关代报不计分（实测多轮），必须客户端完成
        "unforgeable": true,
        "unforgeableReason": "需在库库AI客户端完成对应动作（服务端校验真实行为，网关代上报不计分）",
        "kind": "kuku-activity",
        "source": "activity_kuku",
    })
}

/// 真实「签到」（kukuai30 体系 —— 客户端左下角签到面板的同款接口）。
///
/// ```text
/// POST /act/v2/kukuai30/homepage?uid=<uk>&tid=291651792210   面板/日历
/// POST /act/v2/kukuai30/signin?uid=<uk>&tid=291651792210     签到动作
/// ```
/// `uid` 是会话三件套里的 `uk`（Baidu 数字用户 id），`tid` 是客户端内置
/// 生产配置的 signin30Tid。**无 jsToken 要求**（与 activity/freepoint 两族
/// 不同），实测网关侧可完整执行：签到后 `signed_today` 翻转、连签天数与
/// 下一奖励（网盘会员天卡等）照常推进。成败一律以 homepage 的
/// `signed_today` 为准；动作若被服务端拒绝（errno=8001「账号异常，无法
/// 参与活动」，活动侧风控 —— 2026-11-03 实测此时账号并未签上，旧注释
/// 把它当「已签」是错的），把 errmsg 如实透出。
///
/// 返回归一后的 `{signedToday, continuousDays, nextRewardDays,
/// nextRewardAward, alreadySigned}`；读取失败返回 Err。
/// act（kukuai30）请求头：在标准头基础上把 UA 换成客户端同款
/// （`ACT_USER_AGENT`，理由见该常量与 `do_signin` 的 8001 教训）。
fn act_headers(prepared: &credentials::KukuCredentials) -> Vec<(String, String)> {
    credentials::request_headers(prepared)
        .into_iter()
        .map(|(name, value)| {
            if name.eq_ignore_ascii_case("User-Agent") {
                (name, super::ACT_USER_AGENT.to_string())
            } else {
                (name, value)
            }
        })
        .collect()
}

async fn signin30_homepage_raw(
    credentials: &credentials::KukuCredentials,
    uid: &str,
) -> Result<Value, GatewayError> {
    const SIGNIN30_TID: &str = "291651792210";
    let prepared = credentials::with_business_stoken(credentials).await;
    let headers = act_headers(&prepared);
    let url = format!(
        "{BASE_URL}/act/v2/kukuai30/homepage?uid={uid}&tid={SIGNIN30_TID}&{CLIENT_QUERY}"
    );
    let value = http::post_json_value(&url, &headers, &json!({}), None, Some(30_000)).await?;
    let data = value.get("data").or_else(|| value.get("result")).cloned().unwrap_or(Value::Null);
    if data.is_null() {
        return Err(GatewayError::with_status(502, "KukuAI 签到面板返回为空"));
    }
    Ok(data)
}

/// 读签到状态（homepage 的归一视图）。读取失败返回 Err（调用方按 null 呈现）。
pub async fn signin_state(
    credentials: &credentials::KukuCredentials,
) -> Result<Value, GatewayError> {
    let triple = super::session::ensure_tokens(credentials, None, false).await?;
    let data = signin30_homepage_raw(credentials, triple.uk.trim()).await?;
    Ok(json!({
        "signedToday": data.get("signed_today").and_then(Value::as_bool).unwrap_or(false),
        "continuousDays": data.get("continuous_days").and_then(Value::as_i64).unwrap_or(0),
        "nextRewardDays": data.get("next_reward_days").and_then(Value::as_i64).unwrap_or(0),
        "nextRewardAward": data.get("next_reward_award").and_then(Value::as_str).unwrap_or(""),
        "dailyPoints": 10_i64,
        "lotteryChances": 1_i64,
    }))
}

/// 执行真实签到（kukuai30）。已签（homepage 先判）直接返回 alreadySigned；
/// 动作后无论服务端回什么，都以**复读 homepage 的 signed_today** 为准。
///
/// 动作响应**必须收下**（2026-11-03 教训）：未签时服务端会回 errno=8001
/// 「账号异常，无法参与活动」（活动侧风控拒绝 —— 复读 signed_today=false
/// 证实它不是「已签」，旧注释把它当已签是错的），用户需要看到这个真实
/// 原因而不是一句误导性的「今日已签到」。
///
/// 返回 `{success, alreadySigned, signedToday, continuousDays,
/// nextRewardDays, nextRewardAward, message}`。
pub async fn do_signin(
    credentials: &credentials::KukuCredentials,
) -> Result<Value, GatewayError> {
    let triple = super::session::ensure_tokens(credentials, None, false).await?;
    let uid = triple.uk.trim().to_string();
    // 先看状态：已签就不发动作
    let before = signin30_homepage_raw(credentials, &uid).await?;
    let signed_before = before.get("signed_today").and_then(Value::as_bool).unwrap_or(false);
    if signed_before {
        let mut result = signin_result(&before, false);
        result["alreadySigned"] = json!(true);
        return Ok(result);
    }
    let prepared = credentials::with_business_stoken(credentials).await;
    let headers = act_headers(&prepared);
    let url = format!(
        "{BASE_URL}/act/v2/kukuai30/signin?uid={uid}&tid=291651792210&{CLIENT_QUERY}"
    );
    // 动作响应收下来（errno / errmsg 是拒绝原因的唯一来源）；HTTP 层失败不
    // 在这里断言成败 —— 成败仍以复读 homepage 的 signed_today 为准
    let action = http::post_json_value(&url, &headers, &json!({}), None, Some(30_000))
        .await
        .unwrap_or(Value::Null);
    let action_errno = action.get("errno").and_then(Value::as_i64);
    let action_msg = ["show_msg", "errmsg", "message"]
        .iter()
        .find_map(|key| action.get(key).and_then(Value::as_str))
        .unwrap_or("");
    tokio::time::sleep(std::time::Duration::from_millis(900)).await;
    let after = signin30_homepage_raw(credentials, &uid).await?;
    let signed_now = after.get("signed_today").and_then(Value::as_bool).unwrap_or(false);
    let mut result = signin_result(&after, signed_now);
    result["alreadySigned"] = json!(false);
    if !signed_now {
        // 未签上：把服务端的拒绝原因如实带出（8001 = 账号异常，无法参与
        // 活动 —— 活动侧风控；客户端遇到同一 errno 也会弹同样的文案）。
        // 旧实现此时回「今日已签到」，把失败伪装成了另一种成功。
        result["message"] = json!(match (action_errno, action_msg) {
            (Some(code), "") => format!("服务端拒绝签到（errno {code}）"),
            (Some(code), msg) => format!("服务端拒绝签到（errno {code}：{msg}）"),
            (None, _) => "签到动作未生效（服务端未报错，但面板仍显示未签）".to_string(),
        });
    }
    Ok(result)
}

/// homepage 数据 → 归一签到结果（`just_signed` 决定文案）
fn signin_result(data: &Value, just_signed: bool) -> Value {
    json!({
        "success": data.get("signed_today").and_then(Value::as_bool).unwrap_or(false),
        "alreadySigned": false,
        "signedToday": data.get("signed_today").and_then(Value::as_bool).unwrap_or(false),
        "continuousDays": data.get("continuous_days").and_then(Value::as_i64).unwrap_or(0),
        "nextRewardDays": data.get("next_reward_days").and_then(Value::as_i64).unwrap_or(0),
        "nextRewardAward": data.get("next_reward_award").and_then(Value::as_str).unwrap_or(""),
        "message": if just_signed { "签到成功".to_string() } else { "今日已签到".to_string() },
    })
}

/// 尝试上报一个任务（`POST /api/genflowpro/activity/taskcomplete`）。
///
/// 返回归一后的 `{success, errno, message, reward}`：errno=0 → success=true
/// （reward 取 data.reward_point，可能为 0）；-8 → success=false 且 message
/// 说明需客户端完成；其余 errno 原样透出服务端文案。
pub async fn report_task(
    credentials: &credentials::KukuCredentials,
    task_id_str: &str,
) -> Result<Value, GatewayError> {
    if task_id_str.trim().is_empty() {
        return Err(GatewayError::with_status(400, "缺少任务 ID（task_id_str）"));
    }
    let prepared = credentials::with_business_stoken(credentials).await;
    let headers = credentials::request_headers(&prepared);
    let url = format!(
        "{BASE_URL}/api/genflowpro/activity/taskcomplete?{CLIENT_QUERY}"
    );
    let body = json!({
        "task_from": TASK_FROM,
        "task_id_str": task_id_str.trim(),
        "app_id": APP_ID,
        "clienttype": 401_i64,
        "version": "1.7.3.107",
    });
    let value = http::post_json_value(&url, &headers, &body, None, Some(30_000)).await?;
    let errno = value.get("errno").and_then(Value::as_i64).unwrap_or(-1);
    let message = value
        .get("show_msg")
        .or_else(|| value.get("errmsg"))
        .and_then(Value::as_str)
        .unwrap_or("未知错误")
        .to_string();
    let reward = value
        .get("data")
        .and_then(|data| data.get("reward_point"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if errno == 0 {
        Ok(json!({
            "success": true,
            "errno": 0,
            "message": if reward > 0 { format!("上报成功，+{reward} 积分") } else { "上报成功".to_string() },
            "reward": reward,
        }))
    } else {
        Ok(json!({
            "success": false,
            "errno": errno,
            // -8 = 服务端校验真实行为未发生；errmsg「领取成功」是上游的
            // 通用占位文案，直接透出会误导，这里按实测语义改写
            "message": if errno == -8 {
                "未达成：需在库库AI客户端完成对应动作（对话 / 浏览由客户端计分，网关代上报不计）".to_string()
            } else {
                format!("上报未成功（errno={errno}：{message}）")
            },
            "reward": 0,
        }))
    }
}
