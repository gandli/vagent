//! vagent-api:本地管理 API(127.0.0.1 loopback)。
//! 与 CLI/bot 共享同一套 core;handler 逻辑抽成 handlers.rs 纯函数便于单测。

mod handlers;

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use serde_json::json;
use vagent_core::{load_spec, save_spec, Protocol};

/// 共享状态:配置路径。
pub struct AppState {
    config: PathBuf,
}

type SharedState = Arc<AppState>;

fn default_config() -> PathBuf {
    vagent_core::Spec::default_config_path()
}

#[tokio::main]
async fn main() {
    let config = std::env::var("VAGENT_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_config());

    let state = Arc::new(AppState { config });

    let app = router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:7800")
        .await
        .expect("bind 127.0.0.1:7800");
    println!("vagent-api listening on http://127.0.0.1:7800");
    axum::serve(listener, app).await.expect("server error");
}

/// 构造路由(导出以便将来做 oneshot 测试)。
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/api/status", get(api_status))
        .route("/api/render", get(api_render))
        .route("/api/users", post(api_add_user))
        .with_state(state)
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../ui/index.html"))
}

async fn api_status(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let spec = load_spec(&state.config).map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(handlers::status_view(&spec)))
}

async fn api_render(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use vagent_core::render::xray;
    use vagent_core::spec::Spec;
    let spec = load_spec(&state.config).map_err(|_| StatusCode::NOT_FOUND)?;
    let out = xray::render(&spec, &Spec::base_dir(&state.config))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(out))
}

#[derive(serde::Deserialize)]
struct AddUserBody {
    name: String,
    #[serde(default = "default_port")]
    port: u16,
}

fn default_port() -> u16 {
    443
}

async fn api_add_user(
    State(state): State<SharedState>,
    Json(body): Json<AddUserBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut spec = load_spec(&state.config).map_err(|_| StatusCode::NOT_FOUND)?;
    spec.add_user(&body.name, Protocol::Vless, body.port, true);
    save_spec(&spec, &state.config).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true, "name": body.name })))
}
