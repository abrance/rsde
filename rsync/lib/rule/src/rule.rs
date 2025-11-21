use core::Result;

/// 二进制数据的具体平台类型
#[derive(Debug, Clone, PartialEq)]
pub enum BinaryType {
    Windows,
    Linux,
    Android,
    MacOS,
    iOS,
    Generic, // 通用二进制数据
}

/// 文本数据的具体格式类型
#[derive(Debug, Clone, PartialEq)]
pub enum TextType {
    Markdown,
    Yaml,
    Json,
    Toml,
    Xml,
    Csv,
    PlainText, // 纯文本
}

/// 超文本数据的具体类型
#[derive(Debug, Clone, PartialEq)]
pub enum HyperTextType {
    Html,
    Jsx,
    Vue,
    Generic, // 通用超文本
}

/// 富文本数据的具体格式类型
#[derive(Debug, Clone, PartialEq)]
pub enum RichTextType {
    Markdown,
    Html,
    Rtf,
}

/// 事件类型的层次化定义
#[derive(Debug, Clone, PartialEq)]
pub enum EventType {
    Binary(BinaryType),       // 二进制数据及其子类型
    Text(TextType),           // 文本数据及其子类型
    HyperText(HyperTextType), // 超文本数据及其子类型
    RichText(RichTextType),   // 富文本数据及其子类型
}

impl EventType {
    /// 获取事件类型的字符串表示
    pub fn as_str(&self) -> String {
        match self {
            EventType::Binary(bt) => format!("binary.{:?}", bt).to_lowercase(),
            EventType::Text(tt) => format!("text.{:?}", tt).to_lowercase(),
            EventType::HyperText(ht) => format!("hypertext.{:?}", ht).to_lowercase(),
            EventType::RichText(rt) => format!("richtext.{:?}", rt).to_lowercase(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventMetadata {
    pub id: String,
    pub timestamp: u64,
    pub name: String,
    pub payload_size: usize,
    pub event_type: EventType,
}

/// 每份数据都是一个事件, 包含元数据和实际数据载荷
pub trait CEvent {
    /// 获取事件元数据
    fn get_metadata(&self) -> &EventMetadata;

    /// 获取原始二进制载荷数据
    /// 所有数据统一使用二进制格式存储，具体如何解释由 EventType 决定
    fn get_payload(&self) -> &Vec<u8>;

    /// 尝试将载荷解析为 UTF-8 字符串（适用于文本类型）
    fn get_payload_as_text(&self) -> Option<String> {
        String::from_utf8(self.get_payload().clone()).ok()
    }

    /// 获取载荷的引用（避免克隆）
    fn get_payload_slice(&self) -> &[u8] {
        self.get_payload().as_slice()
    }
}

#[typetag::serde(tag = "source_type")]
pub trait CSource {
    fn outputs(&self) -> Vec<SourceOutput>;

    // build 用于构建数据源实例
    async fn build(&self, cx: SourceContext) -> Result<Source>;

    // can_acknowledge 用于指示数据源是否支持确认机制,如消息队列的 ACK
    fn can_acknowledge(&self) -> bool;
}

#[typetag::serde(tag = "transform_type")]
pub trait CTransform {
    fn build(&self, cx: TransformContext) -> Result<Transform>;

    fn process(&self, data: CEvent) -> Result<CEvent>;
}

#[typetag::serde(tag = "sink_type")]
pub trait CSink {
    fn build(&self, cx: SinkContext) -> Result<Sink>;

    fn write(&self, data: CEvent) -> Result<()>;
}

pub struct DataTransferMetadata {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

pub struct DataTransferConfig {
    pub metadata: DataTransferMetadata,
    pub sources: Vec<Arc<Mutex<dyn CSource>>>,
    pub transforms: Vec<Arc<Mutex<dyn CTransform>>>,
    pub sinks: Vec<Arc<Mutex<dyn CSink>>>,
}
