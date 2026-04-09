use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::models::{
    Bin, BinStatus, Cert, Client, MoveStatus, Order, OrderLine, OrderStatus,
    PreAdvice, PreAdviceStatus, Sku, StagedMove, User, Zone,
};

#[derive(Clone)]
pub struct MockDb {
    pub bins: Vec<Bin>,
    pub skus: Vec<Sku>,
    pub clients: Vec<Client>,
    pub pre_advice: Vec<PreAdvice>,
    pub orders: Vec<Order>,
    pub users: Vec<User>,
    pub staged: Vec<StagedMove>,
}

impl MockDb {
    fn seed() -> Self {
        let bins = vec![
            mk_bin("A-1-1", "A", 1, 1, Zone::Ambient, 300.0, None, BinStatus::Empty),
            mk_bin("A-1-2", "A", 1, 2, Zone::Ambient, 300.0, Some("SKU-001"), BinStatus::Occupied),
            mk_bin("A-1-3", "A", 1, 3, Zone::Ambient, 300.0, None, BinStatus::Empty),
            mk_bin("A-1-4", "A", 1, 4, Zone::Ambient, 300.0, Some("SKU-007"), BinStatus::Occupied),
            mk_bin("A-1-5", "A", 1, 5, Zone::Ambient, 300.0, None, BinStatus::Empty),
            mk_bin("A-2-1", "A", 2, 1, Zone::Ambient, 300.0, None, BinStatus::Reserved),
            mk_bin("A-2-2", "A", 2, 2, Zone::Ambient, 300.0, None, BinStatus::Empty),
            mk_bin("A-2-3", "A", 2, 3, Zone::Ambient, 300.0, Some("SKU-005"), BinStatus::Occupied),
            mk_bin("A-2-4", "A", 2, 4, Zone::Ambient, 300.0, None, BinStatus::Empty),
            mk_bin("A-2-5", "A", 2, 5, Zone::Ambient, 300.0, None, BinStatus::Empty),
            mk_bin("B-1-1", "B", 1, 1, Zone::Cold, 150.0, None, BinStatus::Empty),
            mk_bin("B-1-2", "B", 1, 2, Zone::Cold, 150.0, Some("SKU-002"), BinStatus::Occupied),
            mk_bin("B-1-3", "B", 1, 3, Zone::Cold, 150.0, None, BinStatus::Empty),
            mk_bin("B-1-4", "B", 1, 4, Zone::Cold, 150.0, Some("SKU-004"), BinStatus::Occupied),
            mk_bin("B-1-5", "B", 1, 5, Zone::Cold, 150.0, None, BinStatus::Reserved),
            mk_bin("B-2-1", "B", 2, 1, Zone::Cold, 150.0, None, BinStatus::Empty),
            mk_bin("B-2-2", "B", 2, 2, Zone::Cold, 150.0, None, BinStatus::Empty),
            mk_bin("B-2-3", "B", 2, 3, Zone::Cold, 150.0, Some("SKU-008"), BinStatus::Occupied),
            mk_bin("B-2-4", "B", 2, 4, Zone::Cold, 150.0, None, BinStatus::Empty),
            mk_bin("B-2-5", "B", 2, 5, Zone::Cold, 150.0, None, BinStatus::Empty),
            mk_bin("C-1-1", "C", 1, 1, Zone::Heavy, 2000.0, None, BinStatus::Empty),
            mk_bin("C-1-2", "C", 1, 2, Zone::Heavy, 2000.0, Some("SKU-003"), BinStatus::Occupied),
            mk_bin("C-1-3", "C", 1, 3, Zone::Heavy, 2000.0, None, BinStatus::Empty),
            mk_bin("C-1-4", "C", 1, 4, Zone::Heavy, 2000.0, None, BinStatus::Reserved),
            mk_bin("C-1-5", "C", 1, 5, Zone::Heavy, 2000.0, None, BinStatus::Empty),
            mk_bin("C-2-1", "C", 2, 1, Zone::Heavy, 2000.0, None, BinStatus::Empty),
            mk_bin("C-2-2", "C", 2, 2, Zone::Heavy, 2000.0, Some("SKU-006"), BinStatus::Occupied),
            mk_bin("C-2-3", "C", 2, 3, Zone::Heavy, 2000.0, None, BinStatus::Empty),
            mk_bin("C-2-4", "C", 2, 4, Zone::Heavy, 2000.0, None, BinStatus::Empty),
            mk_bin("C-2-5", "C", 2, 5, Zone::Heavy, 2000.0, None, BinStatus::Empty),
        ];

        let skus = vec![
            Sku { id: "SKU-001".into(), name: "Dry Goods Box A".into(),       weight_kg: 20.0,   temp_zone: Zone::Ambient },
            Sku { id: "SKU-002".into(), name: "Frozen Meals Pack".into(),     weight_kg: 8.0,    temp_zone: Zone::Cold },
            Sku { id: "SKU-003".into(), name: "Steel Plate Bundle".into(),    weight_kg: 800.0,  temp_zone: Zone::Heavy },
            Sku { id: "SKU-004".into(), name: "Chilled Beverages".into(),     weight_kg: 45.0,   temp_zone: Zone::Cold },
            Sku { id: "SKU-005".into(), name: "Office Supplies Box".into(),   weight_kg: 5.0,    temp_zone: Zone::Ambient },
            Sku { id: "SKU-006".into(), name: "Engine Parts Crate".into(),    weight_kg: 1500.0, temp_zone: Zone::Heavy },
            Sku { id: "SKU-007".into(), name: "Cardboard Boxes Stack".into(), weight_kg: 60.0,   temp_zone: Zone::Ambient },
            Sku { id: "SKU-008".into(), name: "Ice Cream Pallet".into(),      weight_kg: 120.0,  temp_zone: Zone::Cold },
        ];

        let clients = vec![
            Client { id: "CLT-001".into(), name: "RetailCo Ltd".into(),       contact: "orders@retailco.com".into(),   active: true },
            Client { id: "CLT-002".into(), name: "FreshFoods GmbH".into(),    contact: "wms@freshfoods.de".into(),     active: true },
            Client { id: "CLT-003".into(), name: "IndustrialCorp".into(),     contact: "logistics@indcorp.com".into(), active: true },
            Client { id: "CLT-004".into(), name: "Archived Client".into(),    contact: "noreply@example.com".into(),   active: false },
        ];

        let pre_advice = vec![
            PreAdvice {
                id: "PA-001".into(), client_id: "CLT-001".into(), sku_id: "SKU-001".into(),
                expected_qty: 50, received_qty: 0,
                expected_date: "2026-04-10".into(), status: PreAdviceStatus::Pending,
            },
            PreAdvice {
                id: "PA-002".into(), client_id: "CLT-002".into(), sku_id: "SKU-002".into(),
                expected_qty: 200, received_qty: 80,
                expected_date: "2026-04-09".into(), status: PreAdviceStatus::Receiving,
            },
            PreAdvice {
                id: "PA-003".into(), client_id: "CLT-002".into(), sku_id: "SKU-004".into(),
                expected_qty: 60, received_qty: 60,
                expected_date: "2026-04-07".into(), status: PreAdviceStatus::Complete,
            },
            PreAdvice {
                id: "PA-004".into(), client_id: "CLT-003".into(), sku_id: "SKU-006".into(),
                expected_qty: 4, received_qty: 0,
                expected_date: "2026-04-12".into(), status: PreAdviceStatus::Pending,
            },
            PreAdvice {
                id: "PA-005".into(), client_id: "CLT-001".into(), sku_id: "SKU-005".into(),
                expected_qty: 100, received_qty: 0,
                expected_date: "2026-04-11".into(), status: PreAdviceStatus::Pending,
            },
        ];

        let orders = vec![
            Order {
                id: "ORD-001".into(), client_id: "CLT-001".into(), status: OrderStatus::Open,
                created_at: 1744060000,
                lines: vec![
                    OrderLine { sku_id: "SKU-001".into(), qty: 5, bin_id: Some("A-1-2".into()) },
                    OrderLine { sku_id: "SKU-005".into(), qty: 2, bin_id: Some("A-2-3".into()) },
                ],
            },
            Order {
                id: "ORD-002".into(), client_id: "CLT-002".into(), status: OrderStatus::Picking,
                created_at: 1744063000,
                lines: vec![
                    OrderLine { sku_id: "SKU-002".into(), qty: 10, bin_id: Some("B-1-2".into()) },
                    OrderLine { sku_id: "SKU-008".into(), qty: 3,  bin_id: Some("B-2-3".into()) },
                ],
            },
            Order {
                id: "ORD-003".into(), client_id: "CLT-003".into(), status: OrderStatus::Shipped,
                created_at: 1744020000,
                lines: vec![
                    OrderLine { sku_id: "SKU-003".into(), qty: 1, bin_id: Some("C-1-2".into()) },
                ],
            },
            Order {
                id: "ORD-004".into(), client_id: "CLT-001".into(), status: OrderStatus::Open,
                created_at: 1744065000,
                lines: vec![
                    OrderLine { sku_id: "SKU-007".into(), qty: 1, bin_id: Some("A-1-4".into()) },
                    OrderLine { sku_id: "SKU-001".into(), qty: 3, bin_id: None },
                ],
            },
        ];

        let users = vec![
            User { id: "USR-001".into(), name: "Alice Chen".into(),     certifications: vec![Cert::Picker] },
            User { id: "USR-002".into(), name: "Bob Rodriguez".into(),  certifications: vec![Cert::Picker, Cert::Forklift] },
            User { id: "USR-003".into(), name: "Carol Smith".into(),    certifications: vec![Cert::Picker, Cert::Forklift, Cert::Supervisor] },
        ];

        Self { bins, skus, clients, pre_advice, orders, users, staged: vec![] }
    }
}

#[allow(clippy::too_many_arguments)]
fn mk_bin(id: &str, aisle: &str, row: u8, col: u8, zone: Zone, cap: f32, sku: Option<&str>, status: BinStatus) -> Bin {
    Bin {
        id: id.into(), aisle: aisle.into(), row, col, zone,
        weight_cap_kg: cap,
        current_sku: sku.map(|s| s.into()),
        status,
    }
}

type Db = Arc<RwLock<MockDb>>;

pub async fn serve(port: u16) {
    let db: Db = Arc::new(RwLock::new(MockDb::seed()));

    let app = Router::new()
        .route("/ords/wms/bins",              get(list_bins))
        .route("/ords/wms/bins/{id}",         get(get_bin))
        .route("/ords/wms/skus",              get(list_skus))
        .route("/ords/wms/skus/{id}",         get(get_sku))
        .route("/ords/wms/inventory/{sku}",   get(get_inventory))
        .route("/ords/wms/clients",           get(list_clients))
        .route("/ords/wms/clients/{id}",      get(get_client))
        .route("/ords/wms/pre-advice",        get(list_pre_advice))
        .route("/ords/wms/pre-advice/{id}",   get(get_pre_advice))
        .route("/ords/wms/orders",            get(list_orders))
        .route("/ords/wms/orders/{id}",       get(get_order))
        .route("/ords/wms/users/{id}",        get(get_user))
        .route("/ords/wms/collections",       get(list_staged).post(create_staged))
        .route("/ords/wms/collections/{id}",  put(update_staged))
        .with_state(db);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ── Bins ─────────────────────────────────────────────────────────────────────

async fn list_bins(State(db): State<Db>) -> Json<Vec<Bin>> {
    Json(db.read().await.bins.clone())
}

async fn get_bin(State(db): State<Db>, Path(id): Path<String>) -> impl IntoResponse {
    match db.read().await.bins.iter().find(|b| b.id == id).cloned() {
        Some(b) => Json(b).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// ── SKUs ─────────────────────────────────────────────────────────────────────

async fn list_skus(State(db): State<Db>) -> Json<Vec<Sku>> {
    Json(db.read().await.skus.clone())
}

async fn get_sku(State(db): State<Db>, Path(id): Path<String>) -> impl IntoResponse {
    match db.read().await.skus.iter().find(|s| s.id == id).cloned() {
        Some(s) => Json(s).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// ── Inventory (dynamic) ───────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct InventoryQuery {
    client_id: Option<String>,
}

#[derive(Serialize)]
struct InventoryResponse {
    sku_id: String,
    on_hand: u32,
    on_order: u32,
    committed: u32,
    available: i32,
    locations: Vec<String>,
    pending_pre_advice: Vec<PreAdviceSummary>,
    open_order_demand: Vec<OrderDemand>,
}

#[derive(Serialize)]
struct PreAdviceSummary {
    id: String,
    client_id: String,
    expected_qty: u32,
    received_qty: u32,
    expected_date: String,
    status: PreAdviceStatus,
}

#[derive(Serialize)]
struct OrderDemand {
    order_id: String,
    client_id: String,
    qty: u32,
    status: OrderStatus,
}

async fn get_inventory(
    State(db): State<Db>,
    Path(sku): Path<String>,
    Query(q): Query<InventoryQuery>,
) -> impl IntoResponse {
    let db = db.read().await;

    let locations: Vec<String> = db.bins.iter()
        .filter(|b| b.current_sku.as_deref() == Some(&sku))
        .map(|b| b.id.clone())
        .collect();
    let on_hand = locations.len() as u32;

    let pending_pre_advice: Vec<PreAdviceSummary> = db.pre_advice.iter()
        .filter(|pa| {
            pa.sku_id == sku
                && matches!(pa.status, PreAdviceStatus::Pending | PreAdviceStatus::Receiving)
                && q.client_id.as_deref().is_none_or(|c| c == pa.client_id)
        })
        .map(|pa| PreAdviceSummary {
            id: pa.id.clone(),
            client_id: pa.client_id.clone(),
            expected_qty: pa.expected_qty,
            received_qty: pa.received_qty,
            expected_date: pa.expected_date.clone(),
            status: pa.status.clone(),
        })
        .collect();
    let on_order: u32 = pending_pre_advice.iter()
        .map(|pa| pa.expected_qty.saturating_sub(pa.received_qty))
        .sum();

    let open_order_demand: Vec<OrderDemand> = db.orders.iter()
        .filter(|o| {
            matches!(o.status, OrderStatus::Open | OrderStatus::Picking)
                && q.client_id.as_deref().is_none_or(|c| c == o.client_id)
        })
        .flat_map(|o| {
            o.lines.iter()
                .filter(|l| l.sku_id == sku)
                .map(|l| OrderDemand {
                    order_id: o.id.clone(),
                    client_id: o.client_id.clone(),
                    qty: l.qty,
                    status: o.status.clone(),
                })
        })
        .collect();
    let committed: u32 = open_order_demand.iter().map(|d| d.qty).sum();

    Json(InventoryResponse {
        sku_id: sku,
        on_hand,
        on_order,
        committed,
        available: on_hand as i32 - committed as i32,
        locations,
        pending_pre_advice,
        open_order_demand,
    }).into_response()
}

// ── Clients ───────────────────────────────────────────────────────────────────

async fn list_clients(State(db): State<Db>) -> Json<Vec<Client>> {
    Json(db.read().await.clients.clone())
}

async fn get_client(State(db): State<Db>, Path(id): Path<String>) -> impl IntoResponse {
    match db.read().await.clients.iter().find(|c| c.id == id).cloned() {
        Some(c) => Json(c).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// ── Pre-advice ────────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct PreAdviceQuery {
    client_id: Option<String>,
    sku_id: Option<String>,
    status: Option<String>,
}

async fn list_pre_advice(
    State(db): State<Db>,
    Query(q): Query<PreAdviceQuery>,
) -> Json<Vec<PreAdvice>> {
    let results = db.read().await.pre_advice.iter()
        .filter(|pa| {
            q.client_id.as_deref().is_none_or(|c| c == pa.client_id)
                && q.sku_id.as_deref().is_none_or(|s| s == pa.sku_id)
                && q.status.as_deref().is_none_or(|s| {
                    format!("{:?}", pa.status).to_lowercase() == s.to_lowercase()
                })
        })
        .cloned()
        .collect();
    Json(results)
}

async fn get_pre_advice(State(db): State<Db>, Path(id): Path<String>) -> impl IntoResponse {
    match db.read().await.pre_advice.iter().find(|p| p.id == id).cloned() {
        Some(p) => Json(p).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// ── Orders ────────────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct OrderQuery {
    client_id: Option<String>,
    status: Option<String>,
}

async fn list_orders(State(db): State<Db>, Query(q): Query<OrderQuery>) -> Json<Vec<Order>> {
    let results = db.read().await.orders.iter()
        .filter(|o| {
            q.client_id.as_deref().is_none_or(|c| c == o.client_id)
                && q.status.as_deref().is_none_or(|s| {
                    format!("{:?}", o.status).to_lowercase() == s.to_lowercase()
                })
        })
        .cloned()
        .collect();
    Json(results)
}

async fn get_order(State(db): State<Db>, Path(id): Path<String>) -> impl IntoResponse {
    match db.read().await.orders.iter().find(|o| o.id == id).cloned() {
        Some(o) => Json(o).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// ── Users ─────────────────────────────────────────────────────────────────────

async fn get_user(State(db): State<Db>, Path(id): Path<String>) -> impl IntoResponse {
    match db.read().await.users.iter().find(|u| u.id == id).cloned() {
        Some(u) => Json(u).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// ── Collections (staged moves) ────────────────────────────────────────────────

async fn list_staged(State(db): State<Db>) -> Json<Vec<StagedMove>> {
    Json(db.read().await.staged.clone())
}

#[derive(Deserialize)]
struct CreateMoveRequest {
    sku_id: String,
    from_bin: String,
    to_bin: String,
    user_id: String,
}

async fn create_staged(State(db): State<Db>, Json(req): Json<CreateMoveRequest>) -> impl IntoResponse {
    let mv = StagedMove {
        id: Uuid::new_v4().to_string(),
        sku_id: req.sku_id,
        from_bin: req.from_bin,
        to_bin: req.to_bin,
        user_id: req.user_id,
        created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        status: MoveStatus::Pending,
    };
    db.write().await.staged.push(mv.clone());
    (StatusCode::CREATED, Json(mv))
}

#[derive(Deserialize)]
struct UpdateMoveRequest {
    status: MoveStatus,
}

async fn update_staged(
    State(db): State<Db>,
    Path(id): Path<String>,
    Json(req): Json<UpdateMoveRequest>,
) -> impl IntoResponse {
    let mut db = db.write().await;
    match db.staged.iter_mut().find(|m| m.id == id) {
        Some(m) => { m.status = req.status; Json(m.clone()).into_response() }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
