use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub seq: i64,
    pub r#type: String,
    #[serde(flatten)]
    pub body: serde_json::Value,
}
