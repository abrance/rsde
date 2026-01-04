use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{HeaderName, HeaderValue};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::time::Duration;

/// HTTP 请求方法
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
    HEAD,
    OPTIONS,
}

impl HttpMethod {
    pub fn as_str(&self) -> &str {
        match self {
            HttpMethod::GET => "GET",
            HttpMethod::POST => "POST",
            HttpMethod::PUT => "PUT",
            HttpMethod::DELETE => "DELETE",
            HttpMethod::PATCH => "PATCH",
            HttpMethod::HEAD => "HEAD",
            HttpMethod::OPTIONS => "OPTIONS",
        }
    }
}

/// HTTP 请求体类型
#[derive(Debug, Clone)]
pub enum HttpBody {
    /// 空请求体
    Empty,
    /// 纯文本
    Text(String),
    /// JSON 数据
    Json(serde_json::Value),
    /// 二进制数据
    Binary(Vec<u8>),
    /// URL 编码的表单数据
    Form(HashMap<String, String>),
}

/// HTTP 请求工具
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// 请求 URL
    pub url: String,
    /// HTTP 方法
    pub method: HttpMethod,
    /// 请求头
    pub headers: HashMap<String, String>,
    /// 请求体
    pub body: HttpBody,
    /// 超时时间（秒）
    pub timeout: Option<u64>,
    /// 是否跟随重定向
    pub follow_redirects: bool,
    /// 最大重定向次数
    pub max_redirects: usize,
    /// 是否验证 SSL 证书
    pub verify_ssl: bool,
}

impl HttpRequest {
    /// 创建一个新的 HTTP 请求
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: HttpMethod::GET,
            headers: HashMap::new(),
            body: HttpBody::Empty,
            timeout: Some(30),
            follow_redirects: true,
            max_redirects: 10,
            verify_ssl: true,
        }
    }

    /// 创建一个 GET 请求
    pub fn get(url: impl Into<String>) -> Self {
        Self::new(url).with_method(HttpMethod::GET)
    }

    /// 创建一个 POST 请求
    pub fn post(url: impl Into<String>) -> Self {
        Self::new(url).with_method(HttpMethod::POST)
    }

    /// 创建一个 PUT 请求
    pub fn put(url: impl Into<String>) -> Self {
        Self::new(url).with_method(HttpMethod::PUT)
    }

    /// 创建一个 DELETE 请求
    pub fn delete(url: impl Into<String>) -> Self {
        Self::new(url).with_method(HttpMethod::DELETE)
    }

    /// 创建一个 PATCH 请求
    pub fn patch(url: impl Into<String>) -> Self {
        Self::new(url).with_method(HttpMethod::PATCH)
    }

    /// 设置 HTTP 方法
    pub fn with_method(mut self, method: HttpMethod) -> Self {
        self.method = method;
        self
    }

    /// 添加请求头
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// 批量添加请求头
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers.extend(headers);
        self
    }

    /// 设置 JSON 请求体
    pub fn with_json<T: Serialize>(mut self, json: &T) -> Result<Self, serde_json::Error> {
        let value = serde_json::to_value(json)?;
        self.body = HttpBody::Json(value);
        self.headers
            .insert("Content-Type".to_string(), "application/json".to_string());
        Ok(self)
    }

    /// 设置文本请求体
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.body = HttpBody::Text(text.into());
        self
    }

    /// 设置二进制请求体
    pub fn with_binary(mut self, data: Vec<u8>) -> Self {
        self.body = HttpBody::Binary(data);
        self
    }

    /// 设置表单请求体
    pub fn with_form(mut self, form: HashMap<String, String>) -> Self {
        self.body = HttpBody::Form(form);
        self.headers.insert(
            "Content-Type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        );
        self
    }

    /// 设置超时时间（秒）
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// 设置是否跟随重定向
    pub fn with_follow_redirects(mut self, follow: bool) -> Self {
        self.follow_redirects = follow;
        self
    }

    /// 设置最大重定向次数
    pub fn with_max_redirects(mut self, max: usize) -> Self {
        self.max_redirects = max;
        self
    }

    /// 设置是否验证 SSL 证书
    pub fn with_verify_ssl(mut self, verify: bool) -> Self {
        self.verify_ssl = verify;
        self
    }

    /// 构建 HTTP 客户端
    fn build_client(&self) -> Result<Client, reqwest::Error> {
        let mut client_builder = Client::builder()
            .redirect(if self.follow_redirects {
                reqwest::redirect::Policy::limited(self.max_redirects)
            } else {
                reqwest::redirect::Policy::none()
            })
            .danger_accept_invalid_certs(!self.verify_ssl);

        if let Some(timeout) = self.timeout {
            client_builder = client_builder.timeout(Duration::from_secs(timeout));
        }

        client_builder.build()
    }

    /// 构建请求
    fn build_request(&self, client: &Client) -> Result<RequestBuilder, reqwest::Error> {
        let mut request = match self.method {
            HttpMethod::GET => client.get(&self.url),
            HttpMethod::POST => client.post(&self.url),
            HttpMethod::PUT => client.put(&self.url),
            HttpMethod::DELETE => client.delete(&self.url),
            HttpMethod::PATCH => client.patch(&self.url),
            HttpMethod::HEAD => client.head(&self.url),
            HttpMethod::OPTIONS => client.request(reqwest::Method::OPTIONS, &self.url),
        };

        // 添加请求头
        for (key, value) in &self.headers {
            if let (Ok(name), Ok(val)) = (
                HeaderName::from_bytes(key.as_bytes()),
                HeaderValue::from_str(value),
            ) {
                request = request.header(name, val);
            }
        }

        // 添加请求体
        request = match &self.body {
            HttpBody::Empty => request,
            HttpBody::Text(text) => request.body(text.clone()),
            HttpBody::Json(json) => request.json(json),
            HttpBody::Binary(data) => request.body(data.clone()),
            HttpBody::Form(form) => request.form(form),
        };

        Ok(request)
    }

    /// 发送请求并获取响应
    pub fn send(&self) -> Result<Response, reqwest::Error> {
        let client = self.build_client()?;
        let request = self.build_request(&client)?;
        request.send()
    }

    /// 发送请求并获取文本响应
    pub fn send_text(&self) -> Result<String, reqwest::Error> {
        let response = self.send()?;
        response.text()
    }

    /// 发送请求并获取 JSON 响应
    pub fn send_json<T: DeserializeOwned>(&self) -> Result<T, reqwest::Error> {
        let response = self.send()?;
        response.json()
    }

    /// 发送请求并获取二进制响应
    pub fn send_bytes(&self) -> Result<Vec<u8>, reqwest::Error> {
        let response = self.send()?;
        Ok(response.bytes()?.to_vec())
    }
}

impl Default for HttpRequest {
    fn default() -> Self {
        Self::new("http://localhost")
    }
}
