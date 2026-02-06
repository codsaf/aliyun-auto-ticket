use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::{info, warn};

use crate::config::Config;
use crate::signer::AliyunSigner;

/// 阿里云工单 API 客户端
pub struct WorkorderClient {
    config: Config,
    signer: AliyunSigner,
    http: reqwest::Client,
}

// ---- API 响应结构 ----

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ApiResponse<T> {
    code: Option<i64>,
    message: Option<String>,
    success: Option<bool>,
    data: Option<T>,
    request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ProductDirectory {
    directory_id: Option<u64>,
    directory_name: Option<String>,
    product_list: Option<Vec<Product>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Product {
    product_id: Option<u64>,
    product_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Category {
    category_id: Option<u64>,
    category_name: Option<String>,
}

impl WorkorderClient {
    pub fn new(config: Config) -> Self {
        let signer =
            AliyunSigner::new(config.access_key_id.clone(), config.access_key_secret.clone());
        let http = reqwest::Client::new();
        Self {
            config,
            signer,
            http,
        }
    }

    /// 发送 API 请求
    async fn call_api(
        &self,
        action: &str,
        query_params: &mut BTreeMap<String, String>,
    ) -> Result<String> {
        let nonce = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let host = &self.config.endpoint;
        let url = format!("https://{}/", host);

        // 公共头
        let mut headers = BTreeMap::new();
        headers.insert("host".to_string(), host.clone());
        headers.insert("x-acs-action".to_string(), action.to_string());
        headers.insert("x-acs-version".to_string(), self.config.api_version.clone());
        headers.insert("x-acs-date".to_string(), timestamp);
        headers.insert("x-acs-signature-nonce".to_string(), nonce);

        // 对于 RPC 风格的 GET 请求，body 为空
        let body = "";
        let content_sha256 = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(body.as_bytes());
            hex::encode(hasher.finalize())
        };
        headers.insert("x-acs-content-sha256".to_string(), content_sha256);

        // 签名
        let authorization = self
            .signer
            .sign("GET", query_params, &headers, body)
            .context("签名计算失败")?;

        // 构建 reqwest 请求
        let query_vec: Vec<(String, String)> = query_params
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let resp = self
            .http
            .get(&url)
            .query(&query_vec)
            .header("Host", host.as_str())
            .header("x-acs-action", action)
            .header("x-acs-version", &self.config.api_version)
            .header("x-acs-date", headers.get("x-acs-date").unwrap().as_str())
            .header("x-acs-signature-nonce", headers.get("x-acs-signature-nonce").unwrap().as_str())
            .header("x-acs-content-sha256", headers.get("x-acs-content-sha256").unwrap().as_str())
            .header("Authorization", &authorization)
            .send()
            .await
            .context("HTTP 请求失败")?;

        let status = resp.status();
        let text = resp.text().await.context("读取响应失败")?;

        if !status.is_success() {
            anyhow::bail!("API 返回错误 (HTTP {}): {}", status, text);
        }

        Ok(text)
    }

    /// 查询产品列表，找到轻量应用服务器的 ProductId
    pub async fn find_product_id(&self) -> Result<u64> {
        info!("正在查询阿里云产品列表...");
        let mut params = BTreeMap::new();
        params.insert("Language".to_string(), "zh".to_string());

        let resp_text = self.call_api("ListProducts", &mut params).await?;
        let resp: ApiResponse<Vec<ProductDirectory>> =
            serde_json::from_str(&resp_text).context("解析 ListProducts 响应失败")?;

        if resp.success != Some(true) {
            anyhow::bail!(
                "ListProducts 失败: {}",
                resp.message.unwrap_or_default()
            );
        }

        let directories = resp.data.context("ListProducts 返回数据为空")?;

        // 搜索轻量应用服务器
        for dir in &directories {
            if let Some(products) = &dir.product_list {
                for product in products {
                    if let Some(name) = &product.product_name {
                        if name.contains("轻量") || name.contains("Simple Application") {
                            let pid = product.product_id.context("产品ID为空")?;
                            info!("找到轻量应用服务器: {} (ProductId: {})", name, pid);
                            return Ok(pid);
                        }
                    }
                }
            }
        }

        // 没找到的话打印所有产品方便调试
        warn!("未找到轻量应用服务器，列出所有产品:");
        for dir in &directories {
            let dir_name = dir.directory_name.as_deref().unwrap_or("未知");
            if let Some(products) = &dir.product_list {
                for product in products {
                    let name = product.product_name.as_deref().unwrap_or("未知");
                    let pid = product.product_id.unwrap_or(0);
                    warn!("  [{dir_name}] {name} (ProductId: {pid})");
                }
            }
        }

        anyhow::bail!("未找到轻量应用服务器产品，请手动设置 TICKET_PRODUCT_ID 环境变量")
    }

    /// 查询工单分类，找到合适的 CategoryId
    pub async fn find_category_id(&self, product_id: u64) -> Result<u64> {
        info!("正在查询工单分类 (ProductId: {})...", product_id);
        let mut params = BTreeMap::new();
        params.insert("ProductId".to_string(), product_id.to_string());
        params.insert("Language".to_string(), "zh".to_string());

        let resp_text = self.call_api("ListCategories", &mut params).await?;
        let resp: ApiResponse<Vec<Category>> =
            serde_json::from_str(&resp_text).context("解析 ListCategories 响应失败")?;

        if resp.success != Some(true) {
            anyhow::bail!(
                "ListCategories 失败: {}",
                resp.message.unwrap_or_default()
            );
        }

        let categories = resp.data.context("ListCategories 返回数据为空")?;

        // 优先选择含有"网络"、"带宽"等关键词的分类
        let keywords = ["带宽", "网络", "限速", "bandwidth", "network"];
        for category in &categories {
            if let Some(name) = &category.category_name {
                let lower = name.to_lowercase();
                for kw in &keywords {
                    if lower.contains(kw) {
                        let cid = category.category_id.context("分类ID为空")?;
                        info!("找到匹配的工单分类: {} (CategoryId: {})", name, cid);
                        return Ok(cid);
                    }
                }
            }
        }

        // 没有精确匹配就用第一个分类
        if let Some(first) = categories.first() {
            let cid = first.category_id.context("分类ID为空")?;
            let name = first.category_name.as_deref().unwrap_or("未知");
            warn!("未找到网络/带宽相关分类，使用第一个分类: {} (CategoryId: {})", name, cid);

            info!("所有可用分类:");
            for cat in &categories {
                info!(
                    "  {} (CategoryId: {})",
                    cat.category_name.as_deref().unwrap_or("未知"),
                    cat.category_id.unwrap_or(0)
                );
            }

            return Ok(cid);
        }

        anyhow::bail!("未找到任何工单分类，请手动设置 TICKET_CATEGORY_ID 环境变量")
    }

    /// 提交工单
    pub async fn create_ticket(&self, category_id: u64) -> Result<String> {
        info!("正在提交工单...");
        let mut params = BTreeMap::new();
        params.insert("CategoryId".to_string(), category_id.to_string());
        params.insert("Severity".to_string(), "2".to_string()); // 2=紧急(业务受损)
        params.insert("Title".to_string(), self.config.ticket_title.clone());
        params.insert(
            "Description".to_string(),
            self.config.ticket_description.clone(),
        );

        let resp_text = self.call_api("CreateTicket", &mut params).await?;

        // CreateTicket 返回的 Data 是工单 ID 字符串
        let resp: ApiResponse<String> =
            serde_json::from_str(&resp_text).context("解析 CreateTicket 响应失败")?;

        if resp.success != Some(true) {
            anyhow::bail!(
                "CreateTicket 失败: {}",
                resp.message.unwrap_or_default()
            );
        }

        let ticket_id = resp.data.context("工单ID为空")?;
        info!("工单提交成功！工单号: {}", ticket_id);
        Ok(ticket_id)
    }

    /// 执行完整的提交工单流程
    pub async fn submit_ticket(&self) -> Result<String> {
        // 1. 确定 ProductId
        let product_id = if self.config.product_id > 0 {
            info!("使用配置的 ProductId: {}", self.config.product_id);
            self.config.product_id
        } else {
            self.find_product_id().await?
        };

        // 2. 确定 CategoryId
        let category_id = if self.config.category_id > 0 {
            info!("使用配置的 CategoryId: {}", self.config.category_id);
            self.config.category_id
        } else {
            self.find_category_id(product_id).await?
        };

        // 3. 提交工单
        self.create_ticket(category_id).await
    }
}
