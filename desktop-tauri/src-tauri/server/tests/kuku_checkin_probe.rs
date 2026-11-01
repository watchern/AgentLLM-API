//! 临时诊断探针 11（跑完即删）：kukuai30 真实签到闭环。
//! homepage（读面板）→ signin（签到动作）→ homepage（验证 signed_today 翻转）。

use agent2api_server::server::config;
use agent2api_server::server::core::account_store::store::AccountStore;
use agent2api_server::server::core::providers::kuku;
use agent2api_server::server::core::proxies::ResolvedProxy;
use serde_json::{json, Value};

const SIGNIN30_TID: &str = "291651792210";

#[test]
fn probe_kukuai30_signin() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let dir = config::config_dir();
        let db = agent2api_server::server::db::Db::open(
            &dir.join(agent2api_server::server::db::FILE_NAME),
        )
        .expect("打开账号库失败");
        let store = AccountStore::with_db(Some(db));
        let list = store.list_accounts();
        let account = list
            .get("accounts")
            .and_then(Value::as_array)
            .unwrap_or(&vec![])
            .iter()
            .find(|item| item.get("provider").and_then(Value::as_str) == Some("kuku"))
            .cloned()
            .expect("没有 kuku 账号");
        let id = account.get("id").and_then(Value::as_str).unwrap_or("");
        let credentials = kuku::credentials::snapshot_for(&store, id).expect("凭证读取");
        let triple =
            kuku::session::ensure_tokens(&credentials, None::<&ResolvedProxy>, false).await.unwrap();
        let prepared = kuku::credentials::with_business_stoken(&credentials).await;
        let headers = kuku::credentials::request_headers(&prepared);
        let uid = triple.uk.trim();
        println!("uid(uk) = {uid}");

        let act = |path: &str| {
            format!(
                "{}/act/v2/kukuai30/{}?uid={}&tid={SIGNIN30_TID}&app_id=123971023&clienttype=401&version=1.7.3.107&channel=chunlei",
                kuku::BASE_URL, path, uid
            )
        };

        // 1) 面板
        let home = kuku::http::post_json_value(&act("homepage"), &headers, &json!({}), None, Some(30_000)).await;
        match &home {
            Ok(value) => {
                let text = serde_json::to_string(value).unwrap_or_default();
                println!(
                    "【homepage】{}",
                    if text.len() > 1400 {
                        format!("{}...(len={})", &text[..1400], text.len())
                    } else {
                        text
                    }
                );
            }
            Err(error) => {
                println!("homepage 失败: {}", error.message);
                return;
            }
        }

        // 2) 签到动作
        match kuku::http::post_json_value(&act("signin"), &headers, &json!({}), None, Some(30_000)).await {
            Ok(value) => println!("【signin】{}", value),
            Err(error) => println!("signin 失败: {}", error.message),
        }

        // 3) 验证
        tokio::time::sleep(std::time::Duration::from_millis(900)).await;
        match kuku::http::post_json_value(&act("homepage"), &headers, &json!({}), None, Some(30_000)).await {
            Ok(value) => {
                let data = value.get("data").or_else(|| value.get("result")).cloned().unwrap_or(Value::Null);
                println!(
                    "签到后 signed_today={} continuous_days={} next_reward_days={} next_reward_award={}",
                    data.get("signed_today").cloned().unwrap_or(Value::Null),
                    data.get("continuous_days").cloned().unwrap_or(Value::Null),
                    data.get("next_reward_days").cloned().unwrap_or(Value::Null),
                    data.get("next_reward_award").cloned().unwrap_or(Value::Null),
                );
            }
            Err(error) => println!("homepage 复读失败: {}", error.message),
        }
    });
}
