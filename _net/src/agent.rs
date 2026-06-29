use std::sync::Arc;
use std::time::Duration;

use ureq::Agent;

/// HTTP 客户端指纹，控制 User-Agent / Accept / Sec-CH-UA 等请求头。
///
/// 不同 CDN 和下载服务器对不同指纹的反应不同，提供多个指纹以应对反爬。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fingerprint {
    /// Chrome 120 on Windows — 兼容性最广，默认使用。
    Chrome120,
    /// Chrome 130 on Windows — 更新的 Chrome 版本。
    Chrome130,
    /// Firefox 134 on Windows — 部分 CDN 对 Chrome 类 UA 有反爬。
    Firefox,
    /// Safari 18 on Windows — 一些苹果 CDN 偏好。
    Safari,
    /// 用户自定义指纹。
    Custom {
        ua: &'static str,
        accept: &'static str,
    },
}

impl Fingerprint {
    /// 返回对应的 User-Agent 字符串。
    pub fn user_agent(&self) -> &'static str {
        match self {
            Fingerprint::Chrome120 => {
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
            }
            Fingerprint::Chrome130 => {
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36"
            }
            Fingerprint::Firefox => {
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:134.0) \
                 Gecko/20100101 Firefox/134.0"
            }
            Fingerprint::Safari => {
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                 AppleWebKit/605.1.15 (KHTML, like Gecko) \
                 Version/18.0 Safari/605.1.15"
            }
            Fingerprint::Custom { ua, .. } => ua,
        }
    }

    /// 返回 Accept 头。
    pub fn accept(&self) -> &'static str {
        match self {
            Fingerprint::Custom { accept, .. } => accept,
            _ => {
                "text/html,application/xhtml+xml,application/xml;q=0.9,\
                 image/avif,image/webp,image/apng,*/*;q=0.8"
            }
        }
    }
}

/// Agent 构建配置。
pub struct AgentConfig {
    /// 使用的浏览器指纹。
    pub fingerprint: Fingerprint,
    /// 是否跳过证书校验（! 不安全，可能受 MITM 攻击）。
    pub insecure: bool,
    /// 连接超时（秒）。
    pub connect_timeout: u64,
    /// 读取超时（秒）。
    pub read_timeout: u64,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            fingerprint: Fingerprint::Chrome120,
            insecure: false,
            connect_timeout: 30,
            read_timeout: 600,
        }
    }
}

impl AgentConfig {
    /// 构建 ureq Agent。
    pub fn build_agent(&self) -> anyhow::Result<Agent> {
        let mut builder = ureq::AgentBuilder::new()
            .user_agent(self.fingerprint.user_agent())
            .timeout_connect(Duration::from_secs(self.connect_timeout))
            .timeout_read(Duration::from_secs(self.read_timeout));

        if self.insecure {
            let tls = native_tls::TlsConnector::builder()
                .danger_accept_invalid_certs(true)
                .build()
                .map_err(|e| anyhow::anyhow!("无法创建 TLS 连接器: {}", e))?;
            builder = builder.tls_connector(Arc::new(tls));
        }

        Ok(builder.build())
    }

    /// 快速构建一个 insecure agent（跳过证书验证）。
    pub fn insecure(connect_timeout: u64, read_timeout: u64) -> anyhow::Result<Agent> {
        Self {
            fingerprint: Fingerprint::Chrome120,
            insecure: true,
            connect_timeout,
            read_timeout,
        }
        .build_agent()
    }

    /// 快速构建一个 normal agent 配置（不跳过证书验证）。
    pub fn normal(connect_timeout: u64, read_timeout: u64) -> Self {
        Self {
            connect_timeout,
            read_timeout,
            insecure: false,
            ..Default::default()
        }
    }

    // ── 请求头辅助 ──

    /// 构建完整的浏览器模拟请求头，包括 `Sec-Fetch-*` / `Sec-CH-UA-*` / `Referer` 等。
    ///
    /// 返回 (header_name, header_value) 列表。
    pub fn browser_headers(&self, url: &str) -> Vec<(&'static str, String)> {
        let mut headers: Vec<(&'static str, String)> = Vec::with_capacity(16);

        // Accept — 下载场景用 */* 避免被当作 HTML 页面请求
        headers.push(("Accept", "*/*".to_string()));

        // 语言
        headers.push(("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6".to_string()));

        // Sec-CH-UA: 客户端提示（Chrome 专属，部分 CDN 依赖）
        match self.fingerprint {
            Fingerprint::Chrome120 | Fingerprint::Chrome130 => {
                headers.push(("Sec-Ch-Ua", "\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"120\", \"Google Chrome\";v=\"120\"".to_string()));
                headers.push(("Sec-Ch-Ua-Mobile", "?0".to_string()));
                headers.push(("Sec-Ch-Ua-Platform", "\"Windows\"".to_string()));
            }
            Fingerprint::Firefox => {
                // Firefox 不发送 Sec-CH-UA
            }
            Fingerprint::Safari => {
                headers.push(("Sec-Ch-Ua", "\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"120\", \"Google Chrome\";v=\"120\"".to_string()));
                headers.push(("Sec-Ch-Ua-Mobile", "?0".to_string()));
                headers.push(("Sec-Ch-Ua-Platform", "\"Windows\"".to_string()));
            }
            Fingerprint::Custom { .. } => {}
        }

        // Sec-Fetch-* 请求头 — 浏览器安全策略，脚本缺少这些头会被反爬
        headers.push(("Sec-Fetch-Site", "none".to_string()));
        headers.push(("Sec-Fetch-Mode", "navigate".to_string()));
        headers.push(("Sec-Fetch-Dest", "document".to_string()));
        headers.push(("Sec-Fetch-User", "?1".to_string()));

        // Upgrade-Insecure-Requests
        headers.push(("Upgrade-Insecure-Requests", "1".to_string()));

        // DNT / 隐私
        headers.push(("DNT", "1".to_string()));

        // Cache control
        headers.push(("Cache-Control", "no-cache".to_string()));
        headers.push(("Pragma", "no-cache".to_string()));

        // Referer — 按域名定制
        let hostname = url
            .split("://")
            .nth(1)
            .and_then(|s| s.split('/').next())
            .unwrap_or("");

        if !hostname.is_empty() {
            // 部分 CDN 防盗链要求 Referer 为原始站点，而非 CDN 域名
            let referer = if hostname.contains("baidupcs.com") {
                "https://pan.baidu.com/".to_string()
            } else {
                match hostname {
                    "download.jetbrains.com" => "https://www.jetbrains.com/".to_string(),
                    "dldir1.qq.com" => "https://work.weixin.qq.com/".to_string(),
                    "softwareupdate.vmware.com" => "https://www.vmware.com/".to_string(),
                    "dl.google.com" => "https://www.google.com/".to_string(),
                    "redirector.gvt1.com" => "https://developer.android.com/".to_string(),
                    "download.trae.com.cn" => "https://www.trae.com.cn/".to_string(),
                    "download.cursor.com" => "https://www.cursor.com/".to_string(),
                    "sunlogin.oray.com" | "dl.oray.com" => "https://sunlogin.oray.com/".to_string(),
                    _ => format!("https://{}/", hostname),
                }
            };
            headers.push(("Referer", referer));
        }

        headers
    }

    /// 将浏览器请求头应用到 ureq Request 上。
    pub fn apply_headers<'a>(&self, req: ureq::Request, url: &str) -> ureq::Request {
        let mut r = req;
        for (key, val) in self.browser_headers(url) {
            r = r.set(key, &val);
        }
        r
    }

    /// 获取浏览器请求头的扁平键值对列表（用于 aria2c `--header` 等外部工具）。
    pub fn flat_headers(&self, url: &str) -> Vec<String> {
        self.browser_headers(url)
            .into_iter()
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect()
    }
}

/// 检测响应是否为 HTML 页面（反盗链页面特征）。
pub fn is_html_response(resp: &ureq::Response) -> bool {
    resp.header("Content-Type")
        .map(|ct| ct.contains("text/html") || ct.contains("text/plain"))
        .unwrap_or(false)
}
