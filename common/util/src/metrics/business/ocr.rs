use metrics::{counter, histogram};

pub struct OcrMetrics;

impl OcrMetrics {
    pub fn record_ocr_success(duration_ms: f64, text_length: usize) {
        counter!("ocr_processing_total").increment(1);
        histogram!("ocr_processing_duration_ms").record(duration_ms);
        histogram!("ocr_text_length").record(text_length as f64);
    }

    pub fn record_ocr_error() {
        counter!("ocr_processing_errors_total").increment(1);
    }

    pub fn record_ocr_engine(engine: &str) {
        let labels = [("engine", engine.to_string())];
        counter!("ocr_engine_usage_total", &labels).increment(1);
    }
}
