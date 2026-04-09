use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::{pyxis_client::PyxisClient, safety_validation, vector_search::VectorIndex};

pub async fn list_skus(pyxis: &PyxisClient) -> String {
    match pyxis.list_skus().await {
        Ok(skus) => serde_json::json!({"skus": skus}).to_string(),
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

pub async fn list_inventory(pyxis: &PyxisClient) -> String {
    match pyxis.list_skus().await {
        Ok(skus) => {
            let mut rows = Vec::new();
            for sku in &skus {
                match pyxis.get_inventory(&sku.id).await {
                    Ok(inv) => rows.push(serde_json::json!({
                        "sku_id": inv.sku_id,
                        "name": sku.name,
                        "on_hand": inv.on_hand,
                        "available": inv.available,
                        "committed": inv.committed,
                        "on_order": inv.on_order,
                        "locations": inv.locations,
                    })),
                    Err(e) => rows.push(serde_json::json!({"sku_id": sku.id, "error": e.to_string()})),
                }
            }
            let total = rows.len();
            serde_json::json!({"inventory": rows, "total_skus": total}).to_string()
        }
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

pub async fn search_skus(index: Arc<Mutex<VectorIndex>>, args: &Value) -> String {
    let query = args["query"].as_str().unwrap_or("").to_string();
    let k = args["limit"].as_u64().unwrap_or(5) as usize;
    match tokio::task::spawn_blocking(move || index.lock().unwrap().search_skus(&query, k)).await {
        Ok(Ok(hits)) => {
            let items: Vec<_> = hits.iter()
                .map(|(score, id)| serde_json::json!({"sku_id": id, "score": score}))
                .collect();
            serde_json::json!({"skus": items}).to_string()
        }
        Ok(Err(e)) => serde_json::json!({"error": e.to_string()}).to_string(),
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

pub async fn search_bins(index: Arc<Mutex<VectorIndex>>, args: &Value) -> String {
    let query = args["query"].as_str().unwrap_or("").to_string();
    let k = args["limit"].as_u64().unwrap_or(5) as usize;
    match tokio::task::spawn_blocking(move || index.lock().unwrap().search(&query, k)).await {
        Ok(Ok(hits)) => {
            let items: Vec<_> = hits.iter()
                .map(|(score, id)| serde_json::json!({"bin_id": id, "score": score}))
                .collect();
            serde_json::json!({"bins": items}).to_string()
        }
        Ok(Err(e)) => serde_json::json!({"error": e.to_string()}).to_string(),
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

pub async fn check_inventory(pyxis: &PyxisClient, args: &Value) -> String {
    let sku_id = args["sku_id"].as_str().unwrap_or("").to_string();
    match pyxis.get_inventory(&sku_id).await {
        Ok(inv) => serde_json::json!({
            "sku_id": inv.sku_id,
            "on_hand": inv.on_hand,
            "on_order": inv.on_order,
            "committed": inv.committed,
            "available": inv.available,
            "locations": inv.locations,
        }).to_string(),
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

pub async fn propose_move(pyxis: &PyxisClient, args: &Value) -> String {
    let sku_id = args["sku_id"].as_str().unwrap_or("").to_string();
    let from_bin_id = args["from_bin_id"].as_str().unwrap_or("").to_string();
    let to_bin_id = args["to_bin_id"].as_str().unwrap_or("").to_string();
    let user_id = args["user_id"].as_str().unwrap_or("").to_string();

    let (bin_res, sku_res, user_res) = tokio::join!(
        pyxis.get_bin(&to_bin_id),
        pyxis.get_sku(&sku_id),
        pyxis.get_user(&user_id),
    );

    let bin = match bin_res { Ok(b) => b, Err(e) => return serde_json::json!({"error": e.to_string()}).to_string() };
    let sku = match sku_res { Ok(s) => s, Err(e) => return serde_json::json!({"error": e.to_string()}).to_string() };
    let user = match user_res { Ok(u) => u, Err(e) => return serde_json::json!({"error": e.to_string()}).to_string() };

    if let Err(e) = safety_validation::validate(&bin, &sku, &user, pyxis).await {
        return serde_json::json!({"error": e.to_string()}).to_string();
    }

    match pyxis.stage_move(&sku_id, &from_bin_id, &to_bin_id, &user_id).await {
        Ok(mv) => serde_json::json!({
            "staged": true,
            "move_id": mv.id,
            "message": "Move staged. 10-second safety window before WMS commit."
        }).to_string(),
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}
