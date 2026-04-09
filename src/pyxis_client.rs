use crate::models::{Bin, MoveStatus, Sku, StagedMove, User};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct InventoryResponse {
    pub sku_id: String,
    pub on_hand: u32,
    pub on_order: u32,
    pub committed: u32,
    pub available: i32,
    pub locations: Vec<String>,
}

pub struct PyxisClient {
    base_url: String,
    http: reqwest::Client,
}

impl PyxisClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    pub async fn list_bins(&self) -> Result<Vec<Bin>> {
        Ok(self.http.get(self.url("/ords/wms/bins")).send().await?.json().await?)
    }

    pub async fn get_bin(&self, id: &str) -> Result<Bin> {
        let resp = self.http.get(self.url(&format!("/ords/wms/bins/{id}"))).send().await?;
        if !resp.status().is_success() { anyhow::bail!("bin '{}' not found (HTTP {})", id, resp.status()); }
        Ok(resp.json().await?)
    }

    pub async fn get_inventory(&self, sku: &str) -> Result<InventoryResponse> {
        let resp = self.http.get(self.url(&format!("/ords/wms/inventory/{sku}"))).send().await?;
        if !resp.status().is_success() { anyhow::bail!("inventory for '{}' not found (HTTP {})", sku, resp.status()); }
        Ok(resp.json().await?)
    }

    pub async fn list_skus(&self) -> Result<Vec<Sku>> {
        Ok(self.http.get(self.url("/ords/wms/skus")).send().await?.json().await?)
    }

    pub async fn get_sku(&self, id: &str) -> Result<Sku> {
        let resp = self.http.get(self.url(&format!("/ords/wms/skus/{id}"))).send().await?;
        if !resp.status().is_success() { anyhow::bail!("SKU '{}' not found (HTTP {})", id, resp.status()); }
        Ok(resp.json().await?)
    }

    pub async fn get_user(&self, id: &str) -> Result<User> {
        let resp = self.http.get(self.url(&format!("/ords/wms/users/{id}"))).send().await?;
        if !resp.status().is_success() { anyhow::bail!("user '{}' not found (HTTP {})", id, resp.status()); }
        Ok(resp.json().await?)
    }

    pub async fn stage_move(&self, sku_id: &str, from_bin: &str, to_bin: &str, user_id: &str) -> Result<StagedMove> {
        #[derive(Serialize)]
        struct Req<'a> { sku_id: &'a str, from_bin: &'a str, to_bin: &'a str, user_id: &'a str }
        Ok(self.http
            .post(self.url("/ords/wms/collections"))
            .json(&Req { sku_id, from_bin, to_bin, user_id })
            .send().await?.json().await?)
    }

    pub async fn list_staged(&self) -> Result<Vec<StagedMove>> {
        Ok(self.http.get(self.url("/ords/wms/collections")).send().await?.json().await?)
    }

    pub async fn update_move(&self, id: &str, status: MoveStatus) -> Result<StagedMove> {
        #[derive(Serialize)]
        struct Req { status: MoveStatus }
        Ok(self.http
            .put(self.url(&format!("/ords/wms/collections/{id}")))
            .json(&Req { status })
            .send().await?.json().await?)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}
