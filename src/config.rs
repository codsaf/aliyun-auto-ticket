use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::info;

/// JSON 配置文件结构（所有字段可选）
#[derive(Debug, Default, Deserialize)]
pub struct FileConfig {
    pub access_key_id: Option<String>,
    pub access_key_secret: Option<String>,
    pub product_id: Option<u64>,
    pub category_id: Option<u64>,
    pub ticket_title: Option<String>,
    pub ticket_description: Option<String>,
    pub cron_expression: Option<String>,
    pub speed_threshold: Option<f64>,
    pub feishu_webhook_url: Option<String>,
    pub callback_url: Option<String>,
    pub callback_port: Option<u16>,
    pub auto_submit: Option<bool>,
}

/// 应用配置
#[derive(Debug, Clone)]
pub struct Config {
    pub access_key_id: String,
    pub access_key_secret: String,
    pub endpoint: String,
    pub api_version: String,
    pub product_id: u64,
    pub category_id: u64,
    pub ticket_title: String,
    pub ticket_description: String,
    pub cron_expression: String,
    pub speed_threshold: f64,
    /// 飞书群机器人 Webhook URL
    pub feishu_webhook_url: Option<String>,
    /// 回调服务的公网基础 URL，如 https://example.com:9876/ticket
    pub callback_url: Option<String>,
    /// 回调服务监听端口
    pub callback_port: u16,
    /// 限速时是否自动提交工单（不等审批）
    pub auto_submit: bool,
}

impl Config {
    pub fn load() -> Result<Self> {
        let file_cfg = Self::load_file();

        let access_key_id = std::env::var("ALIYUN_ACCESS_KEY_ID")
            .ok()
            .or(file_cfg.access_key_id)
            .context("缺少 access_key_id，请在 config.json 或环境变量 ALIYUN_ACCESS_KEY_ID 中设置")?;

        let access_key_secret = std::env::var("ALIYUN_ACCESS_KEY_SECRET")
            .ok()
            .or(file_cfg.access_key_secret)
            .context("缺少 access_key_secret，请在 config.json 或环境变量 ALIYUN_ACCESS_KEY_SECRET 中设置")?;

        let product_id = std::env::var("TICKET_PRODUCT_ID")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(file_cfg.product_id)
            .unwrap_or(0);

        let category_id = std::env::var("TICKET_CATEGORY_ID")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(file_cfg.category_id)
            .unwrap_or(0);

        let ticket_title = std::env::var("TICKET_TITLE")
            .ok()
            .or(file_cfg.ticket_title)
            .unwrap_or_else(|| "香港轻量应用服务器带宽被限速，请帮忙检查解除".to_string());

        let ticket_description = std::env::var("TICKET_DESCRIPTION")
            .ok()
            .or(file_cfg.ticket_description)
            .unwrap_or_else(|| {
                concat!(
                    "您好，我购买的香港轻量应用服务器带宽为30Mbps，",
                    "但目前实际带宽被限制在约10Mbps左右。",
                    "请帮忙检查服务器是否存在带宽限速情况，",
                    "如果存在限速请帮忙解除，恢复到购买时承诺的30Mbps带宽。",
                    "谢谢！"
                )
                .to_string()
            });

        let cron_expression = std::env::var("CRON_EXPRESSION")
            .ok()
            .or(file_cfg.cron_expression)
            .unwrap_or_else(|| "0 0 9 * * *".to_string());

        let speed_threshold = std::env::var("SPEED_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(file_cfg.speed_threshold)
            .unwrap_or(20.0);

        let feishu_webhook_url = std::env::var("FEISHU_WEBHOOK_URL")
            .ok()
            .or(file_cfg.feishu_webhook_url);

        let callback_url = std::env::var("CALLBACK_URL")
            .ok()
            .or(file_cfg.callback_url);

        let callback_port = std::env::var("CALLBACK_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(file_cfg.callback_port)
            .unwrap_or(9876);

        let auto_submit = std::env::var("AUTO_SUBMIT")
            .ok()
            .map(|v| v == "true" || v == "1")
            .or(file_cfg.auto_submit)
            .unwrap_or(false);

        Ok(Self {
            access_key_id,
            access_key_secret,
            endpoint: "workorder.aliyuncs.com".to_string(),
            api_version: "2021-06-10".to_string(),
            product_id,
            category_id,
            ticket_title,
            ticket_description,
            cron_expression,
            speed_threshold,
            feishu_webhook_url,
            callback_url,
            callback_port,
            auto_submit,
        })
    }

    fn load_file() -> FileConfig {
        match std::fs::read_to_string("config.json") {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(cfg) => {
                    info!("已加载 config.json");
                    cfg
                }
                Err(e) => {
                    eprintln!("config.json 解析失败: {}，将忽略配置文件", e);
                    FileConfig::default()
                }
            },
            Err(_) => FileConfig::default(),
        }
    }
}
