use anyhow::Result;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

type HmacSha256 = Hmac<Sha256>;

/// 阿里云 V3 签名 (ACS3-HMAC-SHA256)
pub struct AliyunSigner {
    access_key_id: String,
    access_key_secret: String,
}

impl AliyunSigner {
    pub fn new(access_key_id: String, access_key_secret: String) -> Self {
        Self {
            access_key_id,
            access_key_secret,
        }
    }

    /// 对请求进行签名，返回 Authorization header 值
    pub fn sign(
        &self,
        method: &str,
        query_params: &BTreeMap<String, String>,
        headers: &BTreeMap<String, String>,
        body: &str,
    ) -> Result<String> {
        // 1. 构建 CanonicalQueryString
        let canonical_query_string = Self::build_canonical_query_string(query_params);

        // 2. 构建 CanonicalHeaders 和 SignedHeaders
        // 签名需要包含的 header: host, x-acs-* 开头, content-type
        let mut sign_headers = BTreeMap::new();
        for (key, value) in headers {
            let lower_key = key.to_lowercase();
            if lower_key == "host"
                || lower_key == "content-type"
                || lower_key.starts_with("x-acs-")
            {
                sign_headers.insert(lower_key, value.trim().to_string());
            }
        }

        let canonical_headers: String = sign_headers
            .iter()
            .map(|(k, v)| format!("{}:{}\n", k, v))
            .collect();

        let signed_headers: String = sign_headers
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(";");

        // 3. HashedRequestPayload
        let hashed_payload = Self::sha256_hex(body);

        // 4. 构建 CanonicalRequest
        let canonical_request = format!(
            "{}\n/\n{}\n{}\n{}\n{}",
            method, canonical_query_string, canonical_headers, signed_headers, hashed_payload
        );

        // 5. 构建 StringToSign
        let string_to_sign = format!(
            "ACS3-HMAC-SHA256\n{}",
            Self::sha256_hex(&canonical_request)
        );

        // 6. 计算签名
        let signature = Self::hmac_sha256_hex(&self.access_key_secret, &string_to_sign)?;

        // 7. 构建 Authorization header
        let authorization = format!(
            "ACS3-HMAC-SHA256 Credential={},SignedHeaders={},Signature={}",
            self.access_key_id, signed_headers, signature
        );

        Ok(authorization)
    }

    fn build_canonical_query_string(params: &BTreeMap<String, String>) -> String {
        params
            .iter()
            .map(|(k, v)| {
                format!(
                    "{}={}",
                    Self::percent_encode(k),
                    Self::percent_encode(v)
                )
            })
            .collect::<Vec<_>>()
            .join("&")
    }

    fn percent_encode(s: &str) -> String {
        let mut result = String::new();
        for byte in s.bytes() {
            match byte {
                b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'-'
                | b'_'
                | b'.'
                | b'~' => {
                    result.push(byte as char);
                }
                _ => {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
        result
    }

    fn sha256_hex(data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn hmac_sha256_hex(key: &str, data: &str) -> Result<String> {
        let mut mac =
            HmacSha256::new_from_slice(key.as_bytes()).map_err(|e| anyhow::anyhow!("{}", e))?;
        mac.update(data.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }
}
