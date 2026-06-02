use serde::{Deserialize, Deserializer, de::Error};

#[derive(Debug, Deserialize, Clone)]
pub struct Module {
    pub id: usize,
    pub group: Group,
}
#[derive(Debug, Deserialize, Clone)]
pub struct Group {
    pub name: String,
    #[serde(rename = "type")]
    pub typ: GroupType,
    #[serde(default = "default_group_count")]
    pub count: IntOrString,
    pub label: Option<String>,
    pub desc: Option<String>,
    pub detail: Option<String>,
    pub notes: Option<String>,
    #[serde(default)]
    pub comments: Vec<String>,
    #[serde(default)]
    pub points: Vec<Point>,
    #[serde(default)]
    pub groups: Vec<Group>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Point {
    pub name: String,
    #[serde(rename = "type")]
    pub typ: PointType,
    pub value: Option<IntOrString>,
    pub count: Option<usize>,
    pub size: u16,
    pub sf: Option<IntOrString>,
    pub units: Option<String>,
    #[serde(rename = "access", default, deserialize_with = "de_access")]
    pub writable: bool,
    #[serde(default, deserialize_with = "de_mandatory")]
    pub mandatory: bool,
    #[serde(rename = "static", default, deserialize_with = "de_static")]
    pub is_static: bool,
    pub label: Option<String>,
    pub desc: Option<String>,
    pub detail: Option<String>,
    pub notes: Option<String>,
    #[serde(default)]
    pub comments: Vec<String>,
    #[serde(default)]
    pub symbols: Vec<Symbol>,
    #[serde(default)]
    pub standards: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Symbol {
    pub name: String,
    pub value: serde_json::Value,
    pub label: Option<String>,
    pub desc: Option<String>,
    pub detail: Option<String>,
    pub notes: Option<String>,
    #[serde(default)]
    pub comments: Vec<String>,
}

#[derive(Debug, Deserialize, Copy, Clone)]
#[serde(rename_all = "lowercase")]
pub enum GroupType {
    Group,
    Sync,
}

#[derive(Debug, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PointType {
    Int16,
    Int32,
    Int64,
    Raw16,
    Uint16,
    Uint32,
    Uint64,
    Acc16,
    Acc32,
    Acc64,
    Bitfield16,
    Bitfield32,
    Bitfield64,
    Enum16,
    Enum32,
    Float32,
    Float64,
    String,
    Pad,
    Ipaddr,
    Ipv6addr,
    Eui48,
    Sunssf,
    Count,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum IntOrString {
    Int(i32),
    String(String),
}

fn de_access<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    match s.as_str() {
        "RW" => Ok(true),
        "R" => Ok(false),
        _ => Err(D::Error::custom("expected 'R' or 'RW'")),
    }
}
fn de_mandatory<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    match s.as_str() {
        "M" => Ok(true),
        "O" => Ok(false),
        _ => Err(D::Error::custom("expected 'M' or 'O'")),
    }
}
fn de_static<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    match s.as_str() {
        "S" => Ok(true),
        "D" => Ok(false),
        _ => Err(D::Error::custom("expected 'D' or 'S'")),
    }
}

fn default_group_count() -> IntOrString {
    IntOrString::Int(1)
}
