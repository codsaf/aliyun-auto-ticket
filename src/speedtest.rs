use anyhow::{Context, Result};
use std::time::{Duration, Instant};
use tracing::{info, warn};

const CONFIG_URL: &str = "https://www.speedtest.net/speedtest-config.php";
const SERVER_LIST_URLS: &[&str] = &[
    "https://www.speedtest.net/speedtest-servers-static.php",
    "https://c.speedtest.net/speedtest-servers-static.php",
];

/// 从收到第一个字节起持续下载的时间（秒）
const DOWNLOAD_DURATION_SECS: u64 = 10;
/// 无数据时的超时时间（秒）
const CONNECT_TIMEOUT_SECS: u64 = 30;

/// 从 XML 元素中提取属性值
fn extract_attr(element: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    let start = element.find(&pattern)? + pattern.len();
    let end = start + element[start..].find('"')?;
    Some(element[start..end].to_string())
}

/// Haversine 公式计算两点间球面距离（km）
fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    r * 2.0 * a.sqrt().atan2((1.0 - a).sqrt())
}

struct ClientInfo {
    lat: f64,
    lon: f64,
}

struct Server {
    url: String,
    name: String,
    sponsor: String,
    distance: f64,
}

impl Server {
    fn base_url(&self) -> &str {
        self.url
            .rfind('/')
            .map(|i| &self.url[..i])
            .unwrap_or(&self.url)
    }
}

async fn get_client_info(http: &reqwest::Client) -> Result<ClientInfo> {
    let text = http.get(CONFIG_URL).send().await?.text().await?;

    let start = text
        .find("<client ")
        .context("config XML 中未找到 client 元素")?;
    let end = start + text[start..].find('>').context("client 元素格式错误")?;
    let element = &text[start..=end];

    let lat: f64 = extract_attr(element, "lat")
        .context("缺少 lat")?
        .parse()
        .context("lat 格式错误")?;
    let lon: f64 = extract_attr(element, "lon")
        .context("缺少 lon")?
        .parse()
        .context("lon 格式错误")?;

    Ok(ClientInfo { lat, lon })
}

async fn get_servers(http: &reqwest::Client, client: &ClientInfo) -> Result<Vec<Server>> {
    let mut text = String::new();
    for &url in SERVER_LIST_URLS {
        match http.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                text = resp.text().await.unwrap_or_default();
                if !text.is_empty() {
                    break;
                }
            }
            _ => continue,
        }
    }

    if text.is_empty() {
        anyhow::bail!("无法获取测速服务器列表");
    }

    let mut servers = Vec::new();
    let mut search_from = 0;

    while let Some(pos) = text[search_from..].find("<server ") {
        let abs_start = search_from + pos;
        let end_marker = text[abs_start..]
            .find("/>")
            .or_else(|| text[abs_start..].find('>'));

        if let Some(end) = end_marker {
            let element = &text[abs_start..abs_start + end + 2];

            if let (Some(url), Some(lat_s), Some(lon_s)) = (
                extract_attr(element, "url"),
                extract_attr(element, "lat"),
                extract_attr(element, "lon"),
            ) {
                if let (Ok(lat), Ok(lon)) = (lat_s.parse::<f64>(), lon_s.parse::<f64>()) {
                    let distance = haversine(client.lat, client.lon, lat, lon);
                    servers.push(Server {
                        url,
                        name: extract_attr(element, "name").unwrap_or_default(),
                        sponsor: extract_attr(element, "sponsor").unwrap_or_default(),
                        distance,
                    });
                }
            }

            search_from = abs_start + end + 2;
        } else {
            break;
        }
    }

    servers.sort_by(|a, b| {
        a.distance
            .partial_cmp(&b.distance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(servers)
}

async fn test_latency(http: &reqwest::Client, server: &Server) -> Option<f64> {
    let base = server.base_url();
    let url = format!("{}/latency.txt", base);

    let mut total = 0.0;
    let mut count = 0;

    for _ in 0..3 {
        let start = Instant::now();
        match tokio::time::timeout(Duration::from_secs(5), http.get(&url).send()).await {
            Ok(Ok(resp)) if resp.status().is_success() => {
                let _ = resp.text().await;
                total += start.elapsed().as_secs_f64();
                count += 1;
            }
            _ => {}
        }
    }

    if count > 0 {
        Some(total / count as f64)
    } else {
        None
    }
}

async fn find_best_server(http: &reqwest::Client, servers: &[Server]) -> Result<usize> {
    let top = std::cmp::min(5, servers.len());
    let mut best_idx = 0;
    let mut best_latency = f64::MAX;

    for i in 0..top {
        if let Some(latency) = test_latency(http, &servers[i]).await {
            info!(
                "  {} ({}) - 延迟: {:.1}ms, 距离: {:.0}km",
                servers[i].sponsor,
                servers[i].name,
                latency * 1000.0,
                servers[i].distance
            );
            if latency < best_latency {
                best_latency = latency;
                best_idx = i;
            }
        } else {
            warn!("  {} ({}) - 不可达", servers[i].sponsor, servers[i].name);
        }
    }

    if best_latency == f64::MAX {
        anyhow::bail!("所有候选服务器均不可达");
    }

    Ok(best_idx)
}

/// 执行下载测速，返回下载速度（Mbps）
///
/// 单线程下载，从收到第一个字节开始计时 10 秒。
/// 如果 30 秒内未收到任何数据则超时。
pub async fn measure_download_speed() -> Result<f64> {
    let http = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; speedtest-rust/1.0)")
        .build()?;

    info!("正在获取测速配置...");
    let client_info = get_client_info(&http).await?;
    info!(
        "客户端位置: ({:.2}, {:.2})",
        client_info.lat, client_info.lon
    );

    info!("正在获取服务器列表...");
    let servers = get_servers(&http, &client_info).await?;
    if servers.is_empty() {
        anyhow::bail!("未找到可用的测速服务器");
    }
    info!("找到 {} 个服务器，正在测试延迟...", servers.len());

    let best_idx = find_best_server(&http, &servers).await?;
    let server = &servers[best_idx];
    let base_url = server.base_url().to_string();
    info!(
        "选择服务器: {} ({}) - 距离: {:.0}km",
        server.sponsor, server.name, server.distance
    );

    info!("正在进行下载测速（单线程，{}秒）...", DOWNLOAD_DURATION_SECS);

    let sizes: &[u32] = &[4000, 3500, 3000, 2500, 2000];
    let mut total_bytes: u64 = 0;
    let mut measure_start: Option<Instant> = None;
    let overall_start = Instant::now();
    let mut file_idx = 0;

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

        let size = sizes[file_idx % sizes.len()];
        let url = format!(
            "{}/random{}x{}.jpg?x={}.{}",
            base_url,
            size,
            size,
            chrono::Utc::now().timestamp_millis(),
            file_idx
        );
        file_idx += 1;

        let resp = match http.get(&url).send().await {
            Ok(r) => r,
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
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
