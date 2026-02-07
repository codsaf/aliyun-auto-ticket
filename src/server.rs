use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info};

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
    check_tx: mpsc::Sender<()>,
    secret: Option<String>,
}

impl CallbackServer {
    pub fn new(secret: Option<String>) -> (Self, mpsc::Receiver<()>) {
        let (tx, rx) = mpsc::channel(8);
        let server = Self {
            pending: Arc::new(Mutex::new(Vec::new())),
            check_tx: tx,
            secret,
        };
        (server, rx)
    }

    /// 验证请求中的 secret
    fn verify_secret(&self, params: &HashMap<String, String>) -> bool {
        match &self.secret {
            Some(s) => params.get("secret").map(|v| v == s).unwrap_or(false),
            None => true, // 未配置 secret 则不鉴权
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
    pub fn approve_url(callback_url: &str, token: &str, secret: &Option<String>) -> String {
        let base = callback_url.trim_end_matches('/');
        match secret {
            Some(s) => format!("{}/approve?token={}&secret={}", base, token, s),
            None => format!("{}/approve?token={}", base, token),
        }
    }

    /// 构建手动触发 URL
    pub fn check_url(callback_url: &str, secret: &Option<String>) -> String {
        let base = callback_url.trim_end_matches('/');
        match secret {
            Some(s) => format!("{}/check?secret={}", base, s),
            None => format!("{}/check", base),
        }
    }

    /// 启动 HTTP 服务
    pub async fn start(self: Arc<Self>, port: u16) {
        let app = Router::new()
            .route("/approve", get(handle_approve))
            .route("/check", get(handle_check))
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

/// 处理手动触发
async fn handle_check(
    State(server): State<Arc<CallbackServer>>,
    Query(params): Query<HashMap<String, String>>,
) -> Html<String> {
    if !server.verify_secret(&params) {
        return Html("<h2>❌ 鉴权失败</h2>".to_string());
    }

    match server.check_tx.try_send(()) {
        Ok(_) => {
            info!("收到手动触发请求");
            Html("<h2>✅ 已触发检测，结果将发送到飞书</h2>".to_string())
        }
        Err(_) => {
            Html("<h2>⏳ 已有任务在执行中，请稍后再试</h2>".to_string())
        }
    }
}

async fn handle_approve(
    State(server): State<Arc<CallbackServer>>,
    Query(params): Query<HashMap<String, String>>,
) -> Html<String> {
    if !server.verify_secret(&params) {
        return Html("<h2>❌ 鉴权失败</h2>".to_string());
    }

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
