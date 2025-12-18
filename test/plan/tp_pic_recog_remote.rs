use pic_recog::{recognize_image_by_remote, RemoteOcrConfig};

const REMOTE_OCR_CONFIG_PATH: &str = "../manifest/dev/remote_ocr.toml";
const IMAGE_PATH: &str = "../manifest/dev/tm_1.png";

#[test]
#[ignore]
fn test_remote_image_recognition() {
    let config = match RemoteOcrConfig::from_file(REMOTE_OCR_CONFIG_PATH) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("远程 OCR 配置加载失败: {}", err);
            eprintln!("请确保 {} 文件存在且包含有效的凭证", REMOTE_OCR_CONFIG_PATH);
            return;
        }
    };

    if config.is_placeholder() {
        eprintln!("远程 OCR 配置仍为占位符，请填写真实的凭证后再运行测试");
        return;
    }

    println!("=== 远程 OCR 测试开始 ===");
    println!("配置端点: {}", config.perm_url);
    println!("图片路径: {}", IMAGE_PATH);
    println!(
        "轮询间隔: {}ms, 最大次数: {}",
        config.poll_interval_ms, config.poll_max_attempts
    );

    match recognize_image_by_remote(IMAGE_PATH, &config) {
        Ok(text) => {
            println!("=== 识别成功 ===");
            println!("返回内容长度: {} 字节", text.len());
            println!(
                "返回内容预览（前500字符）:\n{}",
                &text.chars().take(500).collect::<String>()
            );
            assert!(!text.is_empty(), "远程 OCR 返回为空");
        }
        Err(err) => {
            eprintln!("=== 识别失败 ===");
            eprintln!("错误信息: {}", err);
            panic!("远程 OCR 请求失败: {}", err);
        }
    }
}
