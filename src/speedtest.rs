use anyhow::Result;
use std::time::{Duration, Instant};
use tracing::info;

/// 从收到第一个字节起持续下载的时间（秒）
const DOWNLOAD_DURATION_SECS: u64 = 10;
/// 无数据时的超时时间（秒）
const CONNECT_TIMEOUT_SECS: u64 = 30;

/// 测速下载 URL 列表（按优先级排列）
/// 使用大文件以充分利用带宽
const DOWNLOAD_URLS: &[&str] = &[
    // Cloudflare 100MB
    "https://speed.cloudflare.com/__down?bytes=104857600",
    // Cloudflare 25MB（备用）
    "https://speed.cloudflare.com/__down?bytes=26214400",
];

/// 执行下载测速，返回下载速度（Mbps）
///
/// 单线程下载，从收到第一个字节开始计时 10 秒。
/// 如果 30 秒内未收到任何数据则超时。
pub async fn measure_download_speed() -> Result<f64> {
    let http = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .connect_timeout(Duration::from_secs(10))
        .build()?;

    info!(
        "正在进行下载测速（单线程，{}秒）...",
        DOWNLOAD_DURATION_SECS
    );

    let mut total_bytes: u64 = 0;
    let mut measure_start: Option<Instant> = None;
    let overall_start = Instant::now();
    let mut url_idx = 0;

    'outer: loop {
        // 超时检查：30 秒内没收到任何数据
        if measure_start.is_none()
            && overall_start.elapsed() > Duration::from_secs(CONNECT_TIMEOUT_SECS)
        {
            anyhow::bail!("下载测速超时：{}秒内未收到任何数据", CONNECT_TIMEOUT_SECS);
        }

        // 已开始计时，检查是否到 10 秒
        if let Some(start) = measure_start {
            if start.elapsed() >= Duration::from_secs(DOWNLOAD_DURATION_SECS) {
                break;
            }
        }

        let url = DOWNLOAD_URLS[url_idx % DOWNLOAD_URLS.len()];
        // 加随机参数避免缓存
        let url = format!("{}&t={}", url, chrono::Utc::now().timestamp_millis());
        url_idx += 1;

        let resp = match http.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                if url_idx <= DOWNLOAD_URLS.len() {
                    // 第一轮各 URL 都尝试一下
                    continue;
                }
                anyhow::bail!("测速连接失败: {}", e);
            }
        };

        let mut stream = resp;
        loop {
            // 每个 chunk 前检查是否该停了
            if let Some(start) = measure_start {
                if start.elapsed() >= Duration::from_secs(DOWNLOAD_DURATION_SECS) {
                    break 'outer;
                }
            } else if overall_start.elapsed() > Duration::from_secs(CONNECT_TIMEOUT_SECS) {
                anyhow::bail!("下载测速超时：{}秒内未收到任何数据", CONNECT_TIMEOUT_SECS);
            }

            match stream.chunk().await {
                Ok(Some(chunk)) => {
                    // 收到第一个字节时开始计时
                    if measure_start.is_none() {
                        measure_start = Some(Instant::now());
                        info!("数据开始流入，计时开始");
                    }
                    total_bytes += chunk.len() as u64;
                }
                _ => break,
            }
        }
    }

    let elapsed = measure_start
        .map(|s| s.elapsed().as_secs_f64())
        .unwrap_or(0.0);

    if total_bytes == 0 || elapsed == 0.0 {
        anyhow::bail!("下载测速失败：未收到任何数据");
    }

    let speed_mbps = (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0);

    info!(
        "测速完成: 下载 {:.2} MB, 耗时 {:.1}s, 速度 {:.2} Mbps",
        total_bytes as f64 / 1_000_000.0,
        elapsed,
        speed_mbps
    );

    Ok(speed_mbps)
}
