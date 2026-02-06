use anyhow::Result;
use tracing::{error, info};

/// 发送飞书文本消息
pub async fn send_text(webhook_url: &str, text: &str) -> Result<()> {
    let body = serde_json::json!({
        "msg_type": "text",
        "content": { "text": text }
    });

    let resp = reqwest::Client::new()
        .post(webhook_url)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        error!("飞书消息发送失败 ({}): {}", status, text);
        anyhow::bail!("飞书消息发送失败");
    }

    info!("飞书消息已发送");
    Ok(())
}

/// 发送带"提交工单"按钮的飞书交互卡片
pub async fn send_throttle_card(
    webhook_url: &str,
    speed_mbps: f64,
    threshold: f64,
    approve_url: &str,
) -> Result<()> {
    let body = serde_json::json!({
        "msg_type": "interactive",
        "card": {
            "header": {
                "title": { "tag": "plain_text", "content": "⚠️ 带宽限速告警" },
                "template": "red"
            },
            "elements": [
                {
                    "tag": "div",
                    "text": {
                        "tag": "lark_md",
                        "content": format!(
                            "**下载速度**: {:.2} Mbps\n**阈值**: {} Mbps\n**状态**: 低于阈值，疑似被限速",
                            speed_mbps, threshold
                        )
                    }
                },
                { "tag": "hr" },
                {
                    "tag": "action",
                    "actions": [
                        {
                            "tag": "button",
                            "text": { "tag": "plain_text", "content": "提交工单" },
                            "url": approve_url,
                            "type": "primary"
                        }
                    ]
                }
            ]
        }
    });

    let resp = reqwest::Client::new()
        .post(webhook_url)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        error!("飞书卡片发送失败 ({}): {}", status, text);
        anyhow::bail!("飞书卡片发送失败");
    }

    info!("飞书限速告警卡片已发送");
    Ok(())
}
