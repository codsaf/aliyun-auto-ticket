use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::client::WorkorderClient;
use crate::config::Config;
use crate::feishu;

/// 待审批的工单请求
struct PendingApproval {
    token: String,
    config: Config,
    used: bool,
}

pub struct CallbackServer {
    pending: Arc<Mutex<Vec<PendingApproval>>>,
}

impl CallbackServer {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 添加一个待审批请求，返回审批 token
    pub async fn add_pending(&self, config: Config) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        let mut pending = self.pending.lock().await;
        pending.push(PendingApproval {
            token: token.clone(),
            config,
            used: false,
        });
        token
    }

    /// 构建审批 URL
    pub fn approve_url(callback_url: &str, token: &str) -> String {
        let base = callback_url.trim_end_matches('/');
        format!("{}/approve?token={}", base, token)
    }

    /// 启动 HTTP 服务
    pub async fn start(self: Arc<Self>, port: u16) {
        let app = Router::new()
            .route("/approve", get(handle_approve))
            .with_state(self.clone());

        let addr = format!("0.0.0.0:{}", port);
        info!("回调服务已启动: http://{}", addr);

        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                error!("回调服务启动失败: {}", e);
                return;
            }
        };

        if let Err(e) = axum::serve(listener, app).await {
            error!("回调服务异常退出: {}", e);
        }
    }
}

async fn handle_approve(
    State(server): State<Arc<CallbackServer>>,
    Query(params): Query<HashMap<String, String>>,
) -> Html<String> {
    let token = match params.get("token") {
        Some(t) => t.clone(),
        None => return Html("<h2>缺少 token 参数</h2>".to_string()),
    };

    // 查找并标记为已使用
    let config = {
        let mut pending = server.pending.lock().await;
        if let Some(item) = pending.iter_mut().find(|p| p.token == token) {
            if item.used {
                return Html("<h2>✅ 该工单已提交过，请勿重复操作</h2>".to_string());
            }
            item.used = true;
            Some(item.config.clone())
        } else {
            None
        }
    };

    let config = match config {
        Some(c) => c,
        None => return Html("<h2>❌ 无效的 token</h2>".to_string()),
    };

    info!("收到审批回调，正在提交工单...");

    let client = WorkorderClient::new(config.clone());
    match client.submit_ticket().await {
        Ok(ticket_id) => {
            let msg = format!("工单提交成功，工单号: {}", ticket_id);
            info!("{}", msg);

            // 通知飞书
            if let Some(webhook) = &config.feishu_webhook_url {
                let _ = feishu::send_text(webhook, &format!("✅ {}", msg)).await;
            }

            Html(format!("<h2>✅ {}</h2>", msg))
        }
        Err(e) => {
            let msg = format!("工单提交失败: {:#}", e);
            error!("{}", msg);

            if let Some(webhook) = &config.feishu_webhook_url {
                let _ = feishu::send_text(webhook, &format!("❌ {}", msg)).await;
            }

            Html(format!("<h2>❌ {}</h2>", msg))
        }
    }
}
