// TODO CSource: 数据源 Trait
// TODO 考虑一下 CSource 的内容
use core::Result;

#[typetag::serde(tag = "type")]
pub trait CSource {
    fn outputs(&self) -> Vec<SourceOutput>;
    async fn build(&self, cx: SourceContext) -> Result<Source>;
    fn can_acknowledge(&self) -> bool;
}

// #[typetag::serde(tag = "type")]
// pub trait SourceConfig: Debug + NamedComponent + DynClone + Send + Sync {
//     fn outputs(&self) -> Vec<SourceOutput>;
//     async fn build(&self, cx: SourceContext) -> crate::Result<Source>;
//     fn can_acknowledge(&self) -> bool;
// }
