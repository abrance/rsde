use pic_recog::{recognize_image_with_config, OcrConfig};

const TEST_IMAGE_PATH: &str = "../manifest/dev/tm_1.png";
const LOCAL_TESSDATA_DIR: &str = "../manifest/dev/train_data";

#[test]
#[ignore]
fn test_english_recognition() {
    let config = OcrConfig::new()
        .with_language("eng")
        .with_data_path(LOCAL_TESSDATA_DIR);

    let result = recognize_image_with_config(TEST_IMAGE_PATH, &config);
    assert!(result.is_ok());
    assert!(!result.unwrap().is_empty());
}

#[test]
#[ignore]
fn test_chinese_recognition() {
    let config = OcrConfig::new()
        .with_language("chi_sim")
        .with_data_path(LOCAL_TESSDATA_DIR);

    let result = recognize_image_with_config(TEST_IMAGE_PATH, &config);
    assert!(result.is_ok());
    assert!(!result.unwrap().is_empty());
}

#[test]
#[ignore]
fn test_multi_language() {
    let config = OcrConfig::new()
        .with_language("eng+chi_sim")
        .with_data_path(LOCAL_TESSDATA_DIR);

    let result = recognize_image_with_config(TEST_IMAGE_PATH, &config);
    assert!(result.is_ok());
    assert!(!result.unwrap().is_empty());
}
