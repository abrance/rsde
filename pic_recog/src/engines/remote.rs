//! 远程 OCR 引擎
//!
//! 实现通过 HTTP 调用 web.xxxxapp.com 的 OCR 服务

use crate::error::ImageRecognitionError;
use crate::utils::{load_and_validate_remote_image, RemoteImagePayload};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use config::ocr::RemoteOcrConfig;
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::path::Path;
use std::thread::sleep;

const ACCEPT_HEADER_VALUE: &str = "application/json, text/plain, */*";
const CONTENT_TYPE_JSON: &str = "application/json;charset=UTF-8";

/// 调用远程 OCR 服务并返回识别结果
///
/// # 参数
///
/// * `image_path` - 图片文件路径
/// * `config` - 远程 OCR 配置
/// * `include_position` - 是否保留坐标等位置信息（默认 true）
///   - true: 返回包含坐标信息的完整 JSON 结果
///   - false: 仅返回提取的纯文本内容
pub fn recognize(
    image_path: &str,
    config: &RemoteOcrConfig,
    include_position: bool,
) -> Result<String, ImageRecognitionError> {
    let payload = load_and_validate_remote_image(image_path)?;
    let client = build_http_client(config)?;

    let perm_token = request_perm_token(&client, config)?;
    let job_id = start_job(&client, config, &payload, image_path, &perm_token)?;
    let final_snapshot = poll_for_completion(&client, config, &job_id)?;

    if include_position {
        // 返回完整的结果（包含坐标信息）
        extract_full_result(&final_snapshot)
    } else {
        // 仅返回纯文本
        extract_text(&final_snapshot)
            .ok_or_else(|| ImageRecognitionError::EngineError("无法从响应中提取文本".to_string()))
    }
}

fn build_http_client(config: &RemoteOcrConfig) -> Result<Client, ImageRecognitionError> {
    let mut builder = Client::builder().timeout(config.request_timeout());
    if config.accept_invalid_certs {
        builder = builder.danger_accept_invalid_certs(true);
    }

    builder
        .build()
        .map_err(|err| ImageRecognitionError::EngineError(format!("构建 HTTP 客户端失败: {err}")))
}

fn request_perm_token(
    client: &Client,
    config: &RemoteOcrConfig,
) -> Result<String, ImageRecognitionError> {
    let headers = build_perm_headers(config)?;
    let response = execute_json_request(
        client
            .post(&config.perm_url)
            .headers(headers)
            .json(&json!({ "mode": config.mode })),
        "获取远程 OCR token",
    )?;

    first_string(&response, &["/data/token", "/token"]).ok_or_else(|| {
        let brief = extract_brief(&response);
        ImageRecognitionError::EngineError(format!("远程 OCR token 缺失: {brief}"))
    })
}

fn start_job(
    client: &Client,
    config: &RemoteOcrConfig,
    payload: &RemoteImagePayload,
    image_path: &str,
    perm_token: &str,
) -> Result<String, ImageRecognitionError> {
    let headers = build_job_headers(config)?;
    let data_url = build_data_url(payload)?;
    let hash = sha1_hex(&data_url);
    let image_name = Path::new(image_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image");

    let body = json!({
        "token": perm_token,
        "hash": hash,
        "name": image_name,
        "size": payload.bytes.len(),
        "dataUrl": data_url,
        "result": {},
        "status": "processing",
        "isSuccess": false,
    });

    let response = execute_json_request(
        client.post(&config.start_url).headers(headers).json(&body),
        "启动远程 OCR 任务",
    )?;

    first_string(
        &response,
        &[
            "/data/jobStatusId",
            "/data/jobStatusID",
            "/jobStatusId",
            "/data/id",
        ],
    )
    .ok_or_else(|| {
        let brief = extract_brief(&response);
        ImageRecognitionError::EngineError(format!("远程 OCR 返回的任务 ID 缺失: {brief}"))
    })
}

fn poll_for_completion(
    client: &Client,
    config: &RemoteOcrConfig,
    job_id: &str,
) -> Result<Value, ImageRecognitionError> {
    if config.poll_initial_delay_ms > 0 {
        sleep(config.poll_initial_delay());
    }

    let mut attempts: u32 = 0;

    loop {
        let snapshot = fetch_status(client, config, job_id)?;

        if let Some(done) = job_is_finished(&snapshot) {
            if done {
                return Ok(snapshot);
            } else {
                let reason = extract_error_message(&snapshot);
                return Err(ImageRecognitionError::EngineError(format!(
                    "远程 OCR 识别失败: {reason}"
                )));
            }
        }

        attempts += 1;

        if attempts >= config.poll_max_attempts {
            let detail = extract_brief(&snapshot);
            return Err(ImageRecognitionError::EngineError(format!(
                "远程 OCR 轮询超时 (尝试 {} 次): {detail}",
                config.poll_max_attempts
            )));
        }

        sleep(config.poll_interval());
    }
}

fn fetch_status(
    client: &Client,
    config: &RemoteOcrConfig,
    job_id: &str,
) -> Result<Value, ImageRecognitionError> {
    let headers = build_job_headers(config)?;

    execute_json_request(
        client
            .get(&config.status_url)
            .headers(headers)
            .query(&[("jobStatusId", job_id)]),
        "查询远程 OCR 状态",
    )
}

fn execute_json_request(
    builder: RequestBuilder,
    context: &str,
) -> Result<Value, ImageRecognitionError> {
    let response = builder
        .send()
        .map_err(|err| ImageRecognitionError::EngineError(format!("{context} 请求失败: {err}")))?;

    let status = response.status();
    let json_value: Value = response.json().map_err(|err| {
        ImageRecognitionError::EngineError(format!("{context} 响应解析失败: {err}"))
    })?;

    if !status.is_success() {
        let brief = extract_brief(&json_value);
        return Err(ImageRecognitionError::EngineError(format!(
            "{context} 失败，状态码 {status}: {brief}"
        )));
    }

    Ok(json_value)
}

fn build_data_url(payload: &RemoteImagePayload) -> Result<String, ImageRecognitionError> {
    let mime = payload.mime_type()?;
    let encoded = BASE64_STANDARD.encode(&payload.bytes);
    Ok(format!("data:{};base64,{}", mime, encoded))
}

fn sha1_hex(content: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(content.as_bytes());
    let digest = hasher.finalize();
    format!("{:x}", digest)
}

fn build_perm_headers(config: &RemoteOcrConfig) -> Result<HeaderMap, ImageRecognitionError> {
    let mut headers = basic_headers(config)?;
    insert_header(&mut headers, "x-auth-token", &config.auth_token)?;
    insert_header(&mut headers, "x-auth-uuid", &config.auth_uuid)?;
    Ok(headers)
}

fn build_job_headers(config: &RemoteOcrConfig) -> Result<HeaderMap, ImageRecognitionError> {
    let mut headers = build_perm_headers(config)?;
    insert_header(&mut headers, "cookie", &config.auth_cookie)?;
    Ok(headers)
}

fn basic_headers(config: &RemoteOcrConfig) -> Result<HeaderMap, ImageRecognitionError> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static(ACCEPT_HEADER_VALUE));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(CONTENT_TYPE_JSON));
    insert_header(&mut headers, "origin", &config.origin)?;
    let referer = referer_from_origin(&config.origin);
    insert_header(&mut headers, "referer", &referer)?;
    Ok(headers)
}

fn insert_header(
    headers: &mut HeaderMap,
    name: &'static str,
    value: &str,
) -> Result<(), ImageRecognitionError> {
    let header_name = HeaderName::from_static(name);
    let header_value = HeaderValue::from_str(value).map_err(|err| {
        ImageRecognitionError::EngineError(format!("设置请求头 {name} 失败: {err}"))
    })?;
    headers.insert(header_name, header_value);
    Ok(())
}

fn job_is_finished(snapshot: &Value) -> Option<bool> {
    // 优先检查 isEnded 字段（Python 脚本使用此字段）
    if let Some(is_ended) = snapshot.pointer("/data/isEnded").and_then(Value::as_bool) {
        if is_ended {
            // 任务已结束，检查是否成功（code=1 表示成功）
            if let Some(code) = snapshot.pointer("/code").and_then(Value::as_i64) {
                return Some(code == 1);
            }
            // 默认认为已结束即成功
            return Some(true);
        }
        // isEnded 为 false 说明还在处理中
        return None;
    }

    // 备用检查：isSuccess 字段
    if let Some(done) = snapshot
        .pointer("/data/jobStatus/isSuccess")
        .and_then(Value::as_bool)
    {
        return Some(done);
    }

    if let Some(done) = snapshot.pointer("/data/isSuccess").and_then(Value::as_bool) {
        return Some(done);
    }

    // 备用检查：status 字符串
    if let Some(status) = snapshot
        .pointer("/data/jobStatus/status")
        .and_then(Value::as_str)
    {
        return match status {
            "success" | "finished" | "done" => Some(true),
            "failed" | "error" | "cancelled" => Some(false),
            "processing" | "pending" | "init" => None,
            _ => None,
        };
    }

    None
}

fn referer_from_origin(origin: &str) -> String {
    let trimmed = origin.trim_end_matches('/');
    format!("{}/", trimmed)
}

/// 提取包含坐标信息的完整结果
fn extract_full_result(snapshot: &Value) -> Result<String, ImageRecognitionError> {
    // 优先处理 ydResp.words_result 结构（包含坐标）
    if let Some(words_result) = snapshot.pointer("/data/ydResp/words_result") {
        return serde_json::to_string_pretty(words_result).map_err(|err| {
            ImageRecognitionError::EngineError(format!("序列化完整结果失败: {err}"))
        });
    }

    // 备用：返回 data.result 或 data.jobStatus.result
    if let Some(result) = snapshot.pointer("/data/jobStatus/result") {
        return serde_json::to_string_pretty(result).map_err(|err| {
            ImageRecognitionError::EngineError(format!("序列化完整结果失败: {err}"))
        });
    }

    if let Some(result) = snapshot.pointer("/data/result") {
        return serde_json::to_string_pretty(result).map_err(|err| {
            ImageRecognitionError::EngineError(format!("序列化完整结果失败: {err}"))
        });
    }

    // 如果没有找到特定字段，返回整个 data 部分
    if let Some(data) = snapshot.pointer("/data") {
        return serde_json::to_string_pretty(data).map_err(|err| {
            ImageRecognitionError::EngineError(format!("序列化完整结果失败: {err}"))
        });
    }

    // 最后兜底：返回整个响应
    serde_json::to_string_pretty(snapshot).map_err(|err| {
        ImageRecognitionError::EngineError(format!("序列化远程 OCR 结果失败: {err}"))
    })
}

fn extract_text(snapshot: &Value) -> Option<String> {
    // 优先处理 xxxx 的 ydResp.words_result 结构
    if let Some(words_result) = snapshot.pointer("/data/ydResp/words_result") {
        if let Some(array) = words_result.as_array() {
            let mut collected = Vec::new();
            for item in array {
                if let Some(words) = item.get("words").and_then(Value::as_str) {
                    if !words.is_empty() {
                        collected.push(words.to_string());
                    }
                }
            }
            if !collected.is_empty() {
                return Some(collected.join("\n"));
            }
        }
    }

    // 备用路径：常规文本字段
    let candidates = [
        "/data/jobStatus/result/text",
        "/data/result/text",
        "/data/text",
    ];

    for path in candidates.iter() {
        if let Some(node) = snapshot.pointer(path) {
            if let Some(text) = node.as_str() {
                if !text.is_empty() {
                    return Some(text.to_string());
                }
            } else if let Some(array) = node.as_array() {
                let mut collected = Vec::new();
                for item in array {
                    if let Some(s) = item.as_str() {
                        collected.push(s.to_string());
                    } else if let Some(s) = item.get("text").and_then(Value::as_str) {
                        collected.push(s.to_string());
                    } else if let Some(s) = item.get("words").and_then(Value::as_str) {
                        collected.push(s.to_string());
                    }
                }
                if !collected.is_empty() {
                    return Some(collected.join("\n"));
                }
            } else if let Some(obj) = node.as_object() {
                if let Some(text) = obj.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        return Some(text.to_string());
                    }
                }
            }
        }
    }

    // 递归搜索
    if let Some(result) = snapshot.pointer("/data/jobStatus/result") {
        let mut buffer = Vec::new();
        collect_text_nodes(result, &mut buffer);
        if !buffer.is_empty() {
            return Some(buffer.join("\n"));
        }
    }

    None
}

fn collect_text_nodes(node: &Value, acc: &mut Vec<String>) {
    match node {
        Value::String(text) => {
            if !text.is_empty() {
                acc.push(text.to_string());
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_text_nodes(item, acc);
            }
        }
        Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(Value::as_str) {
                if !text.is_empty() {
                    acc.push(text.to_string());
                }
            }

            for value in map.values() {
                collect_text_nodes(value, acc);
            }
        }
        _ => {}
    }
}

fn extract_error_message(snapshot: &Value) -> String {
    let candidates = [
        "/data/jobStatus/errorMessage",
        "/data/jobStatus/message",
        "/data/message",
        "/message",
        "/msg",
    ];

    for path in candidates.iter() {
        if let Some(text) = snapshot.pointer(path).and_then(Value::as_str) {
            if !text.is_empty() {
                return text.to_string();
            }
        }
    }

    extract_brief(snapshot)
}

fn first_string(value: &Value, paths: &[&str]) -> Option<String> {
    for path in paths {
        if let Some(node) = value.pointer(path) {
            if let Some(text) = node.as_str() {
                if !text.is_empty() {
                    return Some(text.to_string());
                }
            } else if let Some(number) = node.as_i64() {
                return Some(number.to_string());
            } else if let Some(number) = node.as_u64() {
                return Some(number.to_string());
            }
        }
    }

    None
}

fn extract_brief(value: &Value) -> String {
    match serde_json::to_string(value) {
        Ok(text) => {
            const MAX_LEN: usize = 256;
            let mut acc = String::new();
            for (idx, ch) in text.chars().enumerate() {
                if idx >= MAX_LEN {
                    acc.push_str("...");
                    break;
                }
                acc.push(ch);
            }
            acc
        }
        Err(_) => "<无法解析的响应>".to_string(),
    }
}
