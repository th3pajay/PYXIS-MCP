use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum Zone {
    Ambient,
    Cold,
    Heavy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum BinStatus {
    Empty,
    Occupied,
    Reserved,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum Cert {
    Picker,
    Forklift,
    Supervisor,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum MoveStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum PreAdviceStatus {
    Pending,
    Receiving,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum OrderStatus {
    Open,
    Picking,
    Shipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bin {
    pub id: String,
    pub aisle: String,
    pub row: u8,
    pub col: u8,
    pub zone: Zone,
    pub weight_cap_kg: f32,
    pub current_sku: Option<String>,
    pub status: BinStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sku {
    pub id: String,
    pub name: String,
    pub weight_kg: f32,
    pub temp_zone: Zone,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    pub id: String,
    pub name: String,
    pub contact: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreAdvice {
    pub id: String,
    pub client_id: String,
    pub sku_id: String,
    pub expected_qty: u32,
    pub received_qty: u32,
    pub expected_date: String,
    pub status: PreAdviceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderLine {
    pub sku_id: String,
    pub qty: u32,
    pub bin_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub client_id: String,
    pub lines: Vec<OrderLine>,
    pub status: OrderStatus,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub certifications: Vec<Cert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedMove {
    pub id: String,
    pub sku_id: String,
    pub from_bin: String,
    pub to_bin: String,
    pub user_id: String,
    pub created_at: u64,
    pub status: MoveStatus,
}
