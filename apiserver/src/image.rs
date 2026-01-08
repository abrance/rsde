use axum::{
    Router,
    extract::{Multipart, State},
    http::StatusCode,
    response::Json,
    routing::post,
};
use config::image_host::ImageHostingConfig;
use lazy_static::lazy_static;
use prometheus::{Counter, Histogram, HistogramOpts, IntCounter};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::io::AsyncWriteExt;
use tracing::{error, info, warn};

lazy_static! {
    /// 成功上传的图片文件数量
    static ref IMAGE_UPLOAD_COUNT: IntCounter = IntCounter::new(
        "image_upload_total",
        "Total number of successfully uploaded images"
    )
    .unwrap();

    /// 上传图片文件大小分布（字节）
    static ref IMAGE_UPLOAD_SIZE: Histogram = Histogram::with_opts(
        HistogramOpts::new(
            "image_upload_size_bytes",
            "Size distribution of uploaded images in bytes"
        )
        .buckets(vec![
            1024.0,        // 1KB
            10240.0,       // 10KB
            102400.0,      // 100KB
            512000.0,      // 500KB
            1048576.0,     // 1MB
            5242880.0,     // 5MB
            10485760.0,    // 10MB
            52428800.0,    // 50MB
        ])
    )
    .unwrap();

    /// 图片清理统计：删除的文件数量
    static ref IMAGE_CLEANUP_COUNT: IntCounter = IntCounter::new(
        "image_cleanup_deleted_total",
        "Total number of deleted images by cleanup task"
    )
    .unwrap();

    /// 图片清理统计：释放的存储空间（字节）
    static ref IMAGE_CLEANUP_SIZE: Counter = Counter::new(
        "image_cleanup_freed_bytes",
        "Total bytes freed by cleanup task"
    )
    .unwrap();
}

/// 注册自定义指标到 prometheus default registry
pub fn register_metrics() -> Result<(), prometheus::Error> {
    let registry = prometheus::default_registry();
    registry.register(Box::new(IMAGE_UPLOAD_COUNT.clone()))?;
    registry.register(Box::new(IMAGE_UPLOAD_SIZE.clone()))?;
    registry.register(Box::new(IMAGE_CLEANUP_COUNT.clone()))?;
    registry.register(Box::new(IMAGE_CLEANUP_SIZE.clone()))?;
    Ok(())
}

#[derive(Clone)]
pub struct ImageState {
    config: Arc<ImageHostingConfig>,
}

impl ImageState {
    pub fn new(config: ImageHostingConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct UploadResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 生成唯一文件名：时间戳 + 随机数
fn generate_filename(extension: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let random: u32 = rand::random();
    format!("{timestamp}_{random:08x}.{extension}")
}

/// 从 content-type 或文件名获取扩展名
fn get_extension(filename: Option<&str>, content_type: Option<&str>) -> String {
    // 先尝试从文件名提取
    if let Some(name) = filename
        && let Some(ext) = name.rsplit('.').next()
        && ext.len() <= 5
        && ext != name
    {
        return ext.to_lowercase();
    }

    // 从 content-type 推断
    match content_type {
        Some("image/png") => "png".to_string(),
        Some("image/jpeg") | Some("image/jpg") => "jpg".to_string(),
        Some("image/gif") => "gif".to_string(),
        Some("image/webp") => "webp".to_string(),
        Some("image/bmp") => "bmp".to_string(),
        _ => "jpg".to_string(), // 默认
    }
}

/// 处理图片上传
pub async fn handle_upload(
    State(state): State<ImageState>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, (StatusCode, String)> {
    info!("收到图片上传请求");

    // 确保存储目录存在
    let storage_path = PathBuf::from(&state.config.storage_dir);
    if !storage_path.exists() {
        tokio::fs::create_dir_all(&storage_path)
            .await
            .map_err(|e| {
                error!("创建存储目录失败: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("创建存储目录失败: {e}"),
                )
            })?;
    }

    // 处理上传的文件
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        error!("读取上传字段失败: {e}");
        (StatusCode::BAD_REQUEST, format!("读取上传字段失败: {e}"))
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name != "file" && name != "files" {
            continue;
        }

        let filename = field.file_name().map(|s| s.to_string());
        let content_type = field.content_type().map(|s| s.to_string());

        info!(
            "处理上传文件: filename={:?}, content_type={:?}",
            filename, content_type
        );

        // 获取扩展名
        let extension = get_extension(filename.as_deref(), content_type.as_deref());

        // 生成唯一文件名
        let new_filename = generate_filename(&extension);
        let file_path = storage_path.join(&new_filename);

        // 读取文件数据
        let data = field.bytes().await.map_err(|e| {
            error!("读取文件数据失败: {e}");
            (StatusCode::BAD_REQUEST, format!("读取文件数据失败: {e}"))
        })?;

        // 保存文件
        let mut file = tokio::fs::File::create(&file_path).await.map_err(|e| {
            error!("创建文件失败: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("创建文件失败: {e}"),
            )
        })?;

        file.write_all(&data).await.map_err(|e| {
            error!("写入文件失败: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("写入文件失败: {e}"),
            )
        })?;

        let file_size = data.len() as u64;
        info!("文件上传成功: {new_filename}, size: {file_size} bytes");

        // 记录 prometheus 指标
        IMAGE_UPLOAD_COUNT.inc();
        IMAGE_UPLOAD_SIZE.observe(file_size as f64);

        // 返回相对路径
        return Ok(Json(UploadResponse {
            success: true,
            path: Some(new_filename),
            error: None,
        }));
    }

    // 没有找到文件字段
    error!("未找到上传文件");
    Err((
        StatusCode::BAD_REQUEST,
        "未找到上传文件（需要 'file' 或 'files' 字段）".to_string(),
    ))
}

/// 清理过期文件
async fn cleanup_expired_files(storage_dir: &str, expire_secs: u64) {
    let storage_path = PathBuf::from(storage_dir);

    if !storage_path.exists() {
        return;
    }

    info!(
        "开始清理过期文件: storage_dir={}, expire_secs={}",
        storage_dir, expire_secs
    );

    let mut total_files = 0;
    let mut deleted_files = 0;
    let mut deleted_size: u64 = 0;

    let now = SystemTime::now();
    let expire_duration = Duration::from_secs(expire_secs);

    match tokio::fs::read_dir(&storage_path).await {
        Ok(mut entries) => {
            while let Ok(Some(entry)) = entries.next_entry().await {
                total_files += 1;
                let path = entry.path();

                // 只处理文件，跳过目录
                if !path.is_file() {
                    continue;
                }

                // 获取文件元数据
                match tokio::fs::metadata(&path).await {
                    Ok(metadata) => {
                        // 检查创建时间
                        if let Ok(created) = metadata.created()
                            && let Ok(age) = now.duration_since(created)
                            && age > expire_duration
                        {
                            // 文件过期，删除
                            let file_size = metadata.len();
                            match tokio::fs::remove_file(&path).await {
                                Ok(_) => {
                                    deleted_files += 1;
                                    deleted_size += file_size;
                                    info!("删除过期文件: {:?}, 年龄: {:?}", path.file_name(), age);
                                }
                                Err(e) => {
                                    error!("删除文件失败 {:?}: {e}", path);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("获取文件元数据失败 {:?}: {e}", path);
                    }
                }
            }

            if deleted_files > 0 {
                info!(
                    "清理完成: 扫描 {total_files} 个文件, 删除 {deleted_files} 个过期文件, 释放 {deleted_size} 字节"
                );

                // 记录清理指标
                IMAGE_CLEANUP_COUNT.inc_by(deleted_files);
                IMAGE_CLEANUP_SIZE.inc_by(deleted_size as f64);
            } else {
                info!("清理完成: 扫描 {total_files} 个文件, 无过期文件");
            }
        }
        Err(e) => {
            error!("读取存储目录失败: {e}");
        }
    }
}

/// 启动定时清理任务
pub fn start_cleanup_task(config: ImageHostingConfig) {
    let storage_dir = config.storage_dir.clone();
    let cleanup_interval = if config.cleanup_interval_secs == 0 {
        3600 // 默认 1 小时
    } else {
        config.cleanup_interval_secs
    };
    let file_expire = if config.file_expire_secs == 0 {
        3600 // 默认 1 小时
    } else {
        config.file_expire_secs
    };

    info!("启动文件清理任务: 间隔={cleanup_interval}秒, 过期时间={file_expire}秒");

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(cleanup_interval));

        loop {
            interval.tick().await;
            cleanup_expired_files(&storage_dir, file_expire).await;
        }
    });
}

/// 创建图片路由
pub fn create_routes(config: ImageHostingConfig) -> Router {
    let state = ImageState::new(config);

    Router::new()
        .route("/upload", post(handle_upload))
        .with_state(state)
}
