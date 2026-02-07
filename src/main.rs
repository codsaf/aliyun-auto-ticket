mod client;
mod config;
mod feishu;
mod templates;
mod server;
mod signer;
mod speedtest;
mod telegram;

use std::sync::Arc;

use anyhow::Result;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info, warn};

struct LocalTimer;

impl tracing_subscriber::fmt::time::FormatTime for LocalTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"))
    }
}

/// 测速并通知飞书
async fn check_speed_and_notify(config: config::Config, callback_server: Arc<server::CallbackServer>) {
    let threshold = config.speed_threshold;
    info!("开始测速，阈值: {} Mbps", threshold);

    let speed = match speedtest::measure_download_speed().await {
        Ok(s) => s,
        Err(e) => {
            error!("测速失败: {:#}", e);
            let msg = format!("测速失败: {:#}\n保险起见请手动检查带宽情况", e);
            if let Some(webhook) = &config.feishu_webhook_url {
                let _ = feishu::send_text(webhook, &msg).await;
            }
            if let (Some(token), Some(chat_id)) = (&config.telegram_bot_token, config.telegram_chat_id) {
                let _ = telegram::send_message(token, chat_id, &format!("❌ {}", msg)).await;
            }
            return;
        }
    };

    // 发送测速结果到飞书
    if speed < threshold {
        warn!("下载速度 {:.2} Mbps 低于阈值 {} Mbps", speed, threshold);

        // 使用多样化模板生成工单内容
        let mut cfg = config.clone();
        cfg.ticket_title = templates::random_title();
        cfg.ticket_description = templates::random_description(speed);
        info!("工单标题: {}", cfg.ticket_title);

        if config.auto_submit {
            // 自动提交模式：直接提交工单
            info!("auto_submit 已开启，直接提交工单");
            let client = client::WorkorderClient::new(cfg.clone());
            match client.submit_ticket().await {
                Ok(ticket_id) => {
                    let msg = format!("⚠️ 带宽限速告警\n下载速度: {:.2} Mbps（阈值: {} Mbps）\n✅ 已自动提交工单: {}", speed, threshold, ticket_id);
                    info!("工单提交成功，工单号: {}", ticket_id);
                    if let Some(webhook) = &cfg.feishu_webhook_url {
                        let _ = feishu::send_text(webhook, &msg).await;
                    }
                    if let (Some(token), Some(chat_id)) = (&cfg.telegram_bot_token, cfg.telegram_chat_id) {
                        let _ = telegram::send_message(token, chat_id, &msg).await;
                    }
                }
                Err(e) => {
                    let msg = format!("⚠️ 带宽限速告警\n下载速度: {:.2} Mbps（阈值: {} Mbps）\n❌ 自动提交工单失败: {:#}", speed, threshold, e);
                    error!("工单提交失败: {:#}", e);
                    if let Some(webhook) = &cfg.feishu_webhook_url {
                        let _ = feishu::send_text(webhook, &msg).await;
                    }
                    if let (Some(token), Some(chat_id)) = (&cfg.telegram_bot_token, cfg.telegram_chat_id) {
                        let _ = telegram::send_message(token, chat_id, &msg).await;
                    }
                }
            }
        } else if cfg.feishu_webhook_url.is_some() {
            // 审批模式：发飞书卡片等待点击
            let webhook = cfg.feishu_webhook_url.clone().unwrap();
            if let Some(callback_url) = cfg.callback_url.clone() {
                let token = callback_server.add_pending(cfg).await;
                let approve_url = server::CallbackServer::approve_url(&callback_url, &token, &config.callback_secret);
                if let Err(e) = feishu::send_throttle_card(&webhook, speed, threshold, &approve_url).await {
                    error!("飞书卡片发送失败: {:#}", e);
                }
            } else {
                let msg = format!(
                    "⚠️ 带宽限速告警\n下载速度: {:.2} Mbps（阈值: {} Mbps）\n未配置 callback_url，请手动提交工单",
                    speed, threshold
                );
                let _ = feishu::send_text(&webhook, &msg).await;
            }
        }
    } else {
        let msg = format!("✅ 测速正常: {:.2} Mbps（阈值: {} Mbps）", speed, threshold);
        info!("{}", msg);
        if let Some(webhook) = &config.feishu_webhook_url {
            let _ = feishu::send_text(webhook, &msg).await;
        }
        if let (Some(token), Some(chat_id)) = (&config.telegram_bot_token, config.telegram_chat_id) {
            let _ = telegram::send_message(token, chat_id, &msg).await;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_timer(LocalTimer)
        .init();

    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("阿里云自动提交工单工具\n");
        println!("用法: {} [选项]\n", args[0]);
        println!("选项:");
        println!("  --now,  -n    立即执行（测速 + 通知 + 等待审批回调）");
        println!("  --submit      直接提交工单（跳过测速）");
        println!("  --speedtest, -s  仅测速，不提交工单");
        println!("  --list, -l    查询产品和分类信息");
        println!("  --help, -h    显示帮助信息");
        println!("\n无参数时进入定时任务模式，按 cron 表达式定期测速并处理。");
        println!("\n配置: 通过 config.json 或环境变量设置，详见 config.example.json");
        return Ok(());
    }

    info!("=== 阿里云自动提交工单工具 ===");
    let config = config::Config::load()?;

    // 直接提交工单（跳过测速）
    if args.iter().any(|a| a == "--submit") {
        info!("直接提交模式（跳过测速）");
        let client = client::WorkorderClient::new(config);
        match client.submit_ticket().await {
            Ok(ticket_id) => info!("工单提交成功，工单号: {}", ticket_id),
            Err(e) => error!("工单提交失败: {:#}", e),
        }
        return Ok(());
    }

    // 查询模式
    if args.iter().any(|a| a == "--list" || a == "-l") {
        info!("查询模式：列出产品和分类信息");
        let client = client::WorkorderClient::new(config);
        match client.find_product_id().await {
            Ok(pid) => {
                info!("轻量应用服务器 ProductId: {}", pid);
                match client.find_category_id(pid).await {
                    Ok(cid) => info!("推荐 CategoryId: {}", cid),
                    Err(e) => error!("查询分类失败: {:#}", e),
                }
            }
            Err(e) => error!("查询产品失败: {:#}", e),
        }
        return Ok(());
    }

    // 仅测速模式
    if args.iter().any(|a| a == "--speedtest" || a == "-s") {
        info!("仅测速模式");
        match speedtest::measure_download_speed().await {
            Ok(speed) => info!("下载速度: {:.2} Mbps", speed),
            Err(e) => error!("测速失败: {:#}", e),
        }
        return Ok(());
    }

    // 创建回调服务
    let (callback_server, trigger_rx) = server::CallbackServer::new(config.callback_secret.clone());
    let callback_server = Arc::new(callback_server);

    // 监听手动触发信号，执行完整流程（测速 → 判断 → 通知/提交工单）
    fn spawn_trigger_listener(
        mut rx: tokio::sync::mpsc::Receiver<()>,
        config: config::Config,
        callback_server: Arc<server::CallbackServer>,
    ) {
        tokio::spawn(async move {
            while rx.recv().await.is_some() {
                check_speed_and_notify(config.clone(), callback_server.clone()).await;
            }
        });
    }

    // 立即执行模式
    if args.iter().any(|a| a == "--now" || a == "-n") {
        info!("立即执行模式");
        let srv = callback_server.clone();
        let port = config.callback_port;
        tokio::spawn(async move { srv.start(port).await });
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // 启动 Telegram Bot
        if config.telegram_bot_token.is_some() {
            let tg_cfg = config.clone();
            tokio::spawn(async move { telegram::start_bot(tg_cfg).await });
        }

        spawn_trigger_listener(trigger_rx, config.clone(), callback_server.clone());
        check_speed_and_notify(config, callback_server).await;

        info!("等待回调或手动触发（按 Ctrl+C 退出）...");
        tokio::signal::ctrl_c().await?;
        return Ok(());
    }

    // 定时任务模式
    info!(
        "定时任务模式，cron: {}，测速阈值: {} Mbps",
        config.cron_expression, config.speed_threshold
    );

    if let Some(url) = &config.callback_url {
        info!("手动触发: {}", server::CallbackServer::check_url(url, &config.callback_secret));
    }

    // 启动回调服务
    let srv = callback_server.clone();
    let port = config.callback_port;
    tokio::spawn(async move { srv.start(port).await });

    // 启动手动触发监听
    spawn_trigger_listener(trigger_rx, config.clone(), callback_server.clone());

    let mut sched = JobScheduler::new().await?;

    let config_for_tg = config.clone();
    let cron_expr = config.cron_expression.clone();
    let cb_server = callback_server.clone();
    let job = Job::new_async_tz(cron_expr.as_str(), chrono::Local, move |_uuid, _lock| {
        let cfg = config.clone();
        let srv = cb_server.clone();
        Box::pin(async move {
            info!("定时任务触发");
            check_speed_and_notify(cfg, srv).await;
        })
    })?;

    sched.add(job).await?;
    sched.start().await?;

    // 启动 Telegram Bot
    if config_for_tg.telegram_bot_token.is_some() {
        tokio::spawn(async move { telegram::start_bot(config_for_tg).await });
    }

    info!("定时任务已启动，等待下次执行...");
    info!("按 Ctrl+C 退出");

    tokio::signal::ctrl_c().await?;
    info!("收到退出信号，正在关闭...");
    sched.shutdown().await?;

    Ok(())
}
