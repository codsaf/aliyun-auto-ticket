use std::sync::Arc;

use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, Me, ParseMode};
use teloxide::utils::command::BotCommands;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::client::WorkorderClient;
use crate::config::Config;
use crate::{speedtest, templates};

/// Bot 共享状态
struct BotState {
    config: Config,
    last_speed: Option<f64>,
    last_check_time: Option<chrono::DateTime<chrono::Local>>,
    start_time: chrono::DateTime<chrono::Local>,
}

type SharedState = Arc<Mutex<BotState>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    #[command(description = "显示帮助信息")]
    Help,
    #[command(description = "立即检测（测速 + 判断 + 提工单）")]
    Check,
    #[command(description = "仅测速，不提交工单")]
    Speed,
    #[command(description = "直接提交工单（跳过测速）")]
    Submit,
    #[command(description = "查看当前状态")]
    Status,
}

/// 检查是否为授权用户
fn is_authorized(config: &Config, chat_id: ChatId) -> bool {
    match config.telegram_chat_id {
        Some(id) => chat_id.0 == id,
        None => true,
    }
}

/// 处理命令
async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    state: SharedState,
) -> HandlerResult {
    let chat_id = msg.chat.id;

    // 权限检查
    {
        let s = state.lock().await;
        if !is_authorized(&s.config, chat_id) {
            warn!("未授权的 Telegram 用户: {}", chat_id);
            return Ok(());
        }
    }

    match cmd {
        Command::Help => {
            let text = "🤖 *阿里云工单助手*\n\n\
                        /check \\- 立即检测（测速→判断→提工单）\n\
                        /speed \\- 仅测速\n\
                        /submit \\- 直接提交工单\n\
                        /status \\- 查看状态\n\
                        /help \\- 显示帮助";
            bot.send_message(chat_id, text)
                .parse_mode(ParseMode::MarkdownV2)
                .await?;
        }

        Command::Check => {
            let s = state.lock().await;
            let threshold = s.config.speed_threshold;
            let auto_submit = s.config.auto_submit;
            drop(s);

            bot.send_message(chat_id, format!("⏳ 正在测速，阈值: {} Mbps ...", threshold))
                .await?;

            match speedtest::measure_download_speed().await {
                Ok(speed) => {
                    // 更新状态
                    {
                        let mut s = state.lock().await;
                        s.last_speed = Some(speed);
                        s.last_check_time = Some(chrono::Local::now());
                    }

                    if speed < threshold {
                        if auto_submit {
                            // 自动提交模式
                            bot.send_message(
                                chat_id,
                                format!(
                                    "⚠️ 下载速度: {:.2} Mbps（低于阈值 {} Mbps）\n正在自动提交工单...",
                                    speed, threshold
                                ),
                            )
                            .await?;

                            let s = state.lock().await;
                            let mut cfg = s.config.clone();
                            drop(s);
                            cfg.ticket_title = templates::random_title();
                            cfg.ticket_description = templates::random_description(speed);

                            let client = WorkorderClient::new(cfg);
                            match client.submit_ticket().await {
                                Ok(ticket_id) => {
                                    bot.send_message(
                                        chat_id,
                                        format!("✅ 工单提交成功，工单号: {}", ticket_id),
                                    )
                                    .await?;
                                }
                                Err(e) => {
                                    bot.send_message(
                                        chat_id,
                                        format!("❌ 工单提交失败: {:#}", e),
                                    )
                                    .await?;
                                }
                            }
                        } else {
                            // 审批模式：发送带按钮的消息
                            let buttons = vec![vec![
                                InlineKeyboardButton::callback("✅ 提交工单", format!("submit:{:.2}", speed)),
                                InlineKeyboardButton::callback("❌ 取消", "cancel"),
                            ]];
                            bot.send_message(
                                chat_id,
                                format!(
                                    "⚠️ 带宽限速告警\n\n下载速度: {:.2} Mbps\n阈值: {} Mbps\n\n是否提交工单？",
                                    speed, threshold
                                ),
                            )
                            .reply_markup(InlineKeyboardMarkup::new(buttons))
                            .await?;
                        }
                    } else {
                        bot.send_message(
                            chat_id,
                            format!("✅ 速度正常: {:.2} Mbps（阈值: {} Mbps）", speed, threshold),
                        )
                        .await?;
                    }
                }
                Err(e) => {
                    bot.send_message(chat_id, format!("❌ 测速失败: {:#}", e))
                        .await?;
                }
            }
        }

        Command::Speed => {
            bot.send_message(chat_id, "⏳ 正在测速...").await?;

            match speedtest::measure_download_speed().await {
                Ok(speed) => {
                    {
                        let mut s = state.lock().await;
                        s.last_speed = Some(speed);
                        s.last_check_time = Some(chrono::Local::now());
                    }
                    bot.send_message(chat_id, format!("📊 下载速度: {:.2} Mbps", speed))
                        .await?;
                }
                Err(e) => {
                    bot.send_message(chat_id, format!("❌ 测速失败: {:#}", e))
                        .await?;
                }
            }
        }

        Command::Submit => {
            let buttons = vec![vec![
                InlineKeyboardButton::callback("✅ 确认提交", "force_submit"),
                InlineKeyboardButton::callback("❌ 取消", "cancel"),
            ]];
            bot.send_message(chat_id, "⚠️ 确认要跳过测速直接提交工单吗？")
                .reply_markup(InlineKeyboardMarkup::new(buttons))
                .await?;
        }

        Command::Status => {
            let s = state.lock().await;
            let uptime = chrono::Local::now() - s.start_time;
            let hours = uptime.num_hours();
            let minutes = uptime.num_minutes() % 60;

            let last_speed_str = match s.last_speed {
                Some(speed) => format!("{:.2} Mbps", speed),
                None => "尚未测速".to_string(),
            };
            let last_time_str = match &s.last_check_time {
                Some(t) => t.format("%Y-%m-%d %H:%M:%S").to_string(),
                None => "无".to_string(),
            };

            let text = format!(
                "📊 *状态信息*\n\n\
                 运行时长: {}h {}m\n\
                 上次测速: {}\n\
                 上次结果: {}\n\
                 速度阈值: {} Mbps\n\
                 自动提交: {}\n\
                 定时任务: {}",
                hours,
                minutes,
                last_time_str,
                last_speed_str,
                s.config.speed_threshold,
                if s.config.auto_submit { "开启" } else { "关闭" },
                s.config.cron_expression
            );
            drop(s);

            bot.send_message(chat_id, text).await?;
        }
    }

    Ok(())
}

/// 处理 Inline 按钮回调
async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    state: SharedState,
) -> HandlerResult {
    let data = match &q.data {
        Some(d) => d.clone(),
        None => return Ok(()),
    };

    let chat_id = match &q.message {
        Some(msg) => msg.chat().id,
        None => return Ok(()),
    };

    // 权限检查
    {
        let s = state.lock().await;
        if !is_authorized(&s.config, chat_id) {
            bot.answer_callback_query(&q.id).await?;
            return Ok(());
        }
    }

    if data == "cancel" {
        bot.answer_callback_query(&q.id).text("已取消").await?;
        bot.send_message(chat_id, "❌ 已取消").await?;
        return Ok(());
    }

    if data.starts_with("submit:") || data == "force_submit" {
        bot.answer_callback_query(&q.id).text("正在提交...").await?;
        bot.send_message(chat_id, "⏳ 正在提交工单...").await?;

        let s = state.lock().await;
        let mut cfg = s.config.clone();
        drop(s);

        // 如果是从 check 流程来的，speed 信息在 data 里
        let speed = if data.starts_with("submit:") {
            data.trim_start_matches("submit:").parse::<f64>().ok()
        } else {
            None
        };

        cfg.ticket_title = templates::random_title();
        cfg.ticket_description = match speed {
            Some(s) => templates::random_description(s),
            None => templates::random_description(0.0),
        };

        let client = WorkorderClient::new(cfg);
        match client.submit_ticket().await {
            Ok(ticket_id) => {
                bot.send_message(chat_id, format!("✅ 工单提交成功，工单号: {}", ticket_id))
                    .await?;
            }
            Err(e) => {
                bot.send_message(chat_id, format!("❌ 工单提交失败: {:#}", e))
                    .await?;
            }
        }
    }

    Ok(())
}

/// 发送消息到 Telegram（供定时任务等外部调用）
pub async fn send_message(token: &str, chat_id: i64, text: &str) -> anyhow::Result<()> {
    let bot = Bot::new(token);
    bot.send_message(ChatId(chat_id), text)
        .await
        .map_err(|e| anyhow::anyhow!("Telegram 发送失败: {}", e))?;
    Ok(())
}

/// 启动 Telegram Bot（long polling 模式）
pub async fn start_bot(config: Config) {
    let token = match &config.telegram_bot_token {
        Some(t) => t.clone(),
        None => return,
    };

    let bot = Bot::new(&token);

    // 验证 token 是否有效
    let me: Me = match bot.get_me().await {
        Ok(me) => me,
        Err(e) => {
            error!("Telegram Bot 启动失败，token 无效: {}", e);
            return;
        }
    };
    info!("Telegram Bot 已启动: @{}", me.username());

    // 注册命令菜单
    if let Err(e) = bot.set_my_commands(Command::bot_commands()).await {
        warn!("设置 Bot 命令菜单失败: {}", e);
    }

    let state: SharedState = Arc::new(Mutex::new(BotState {
        config,
        last_speed: None,
        last_check_time: None,
        start_time: chrono::Local::now(),
    }));

    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(handle_command),
        )
        .branch(
            Update::filter_callback_query()
                .endpoint(handle_callback),
        );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .default_handler(|_upd| async {})
        .build()
        .dispatch()
        .await;
}
