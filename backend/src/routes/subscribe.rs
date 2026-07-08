use crate::db::Database;
use crate::models::{
    ApiResponse, CachedEarthquake, SubscribeRequest, Subscription, TestNotifyRequest,
};
use crate::utils::{distance, intensity};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Serialize;
use std::sync::{Arc, Mutex};

/// 应用状态
#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub latest_earthquake: Arc<Mutex<Option<CachedEarthquake>>>,
}

/// 订阅处理器
pub async fn subscribe_handler(
    State(state): State<AppState>,
    Json(payload): Json<SubscribeRequest>,
) -> impl IntoResponse {
    // 验证输入
    if payload.bark_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<SubscribeResponse>::error("Bark ID 不能为空")),
        );
    }

    // Bark ID 长度限制，防止过长数据
    if payload.bark_id.len() > 64 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<SubscribeResponse>::error(
                "Bark ID 过长（最大64字符）",
            )),
        );
    }

    // 验证 Bark ID 只包含安全字符（字母、数字）
    if !payload.bark_id.chars().all(|c| c.is_alphanumeric()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<SubscribeResponse>::error(
                "Bark ID 只能包含字母、数字",
            )),
        );
    }

    if !distance::validate_coordinates(payload.latitude, payload.longitude) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<SubscribeResponse>::error("无效的经纬度坐标")),
        );
    }

    if !intensity::validate_intensity(payload.min_intensity) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<SubscribeResponse>::error(
                "烈度阈值必须在 0-7 之间",
            )),
        );
    }

    // 创建订阅
    let subscription = Subscription::new(
        payload.bark_id.clone(),
        payload.latitude,
        payload.longitude,
        payload.min_intensity,
        payload.bark_api_url.clone(),
        payload.passive_max,
        payload.active_max,
    );

    // 打印订阅信息
    tracing::info!(
        "收到订阅请求 - Bark ID: {}, 位置: ({:.4}, {:.4}), 最小震度: {}",
        subscription.bark_id,
        subscription.latitude,
        subscription.longitude,
        subscription.min_intensity
    );

    // 保存到数据库
    let store = state.db.subscriptions();
    match store.upsert_subscription(subscription.clone()) {
        Ok(_) => {
            tracing::info!(
                "订阅成功 - Bark ID: {}, GeoHash: {}",
                subscription.bark_id,
                crate::utils::geohash::encode(subscription.latitude, subscription.longitude)
            );
            (
                StatusCode::OK,
                Json(ApiResponse::success(
                    "订阅成功",
                    Some(SubscribeResponse::from(subscription)),
                )),
            )
        }
        Err(e) => {
            tracing::error!(
                "订阅失败 - Bark ID: {}, 错误: {:?}",
                subscription.bark_id,
                e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<SubscribeResponse>::error(format!(
                    "订阅失败: {}",
                    e
                ))),
            )
        }
    }
}

/// 取消订阅处理器（路径参数版本）
pub async fn unsubscribe_by_path_handler(
    State(state): State<AppState>,
    Path(bark_id): Path<String>,
) -> impl IntoResponse {
    // 验证输入
    if bark_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error("Bark ID 不能为空")),
        );
    }

    // Bark ID 长度限制，防止过长数据
    if bark_id.len() > 256 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error("Bark ID 过长（最大256字符）")),
        );
    }

    // 验证 Bark ID 只包含安全字符（字母、数字、下划线、连字符）
    if !bark_id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                "Bark ID 只能包含字母、数字、下划线和连字符",
            )),
        );
    }

    tracing::info!("收到取消订阅请求（路径参数）- Bark ID: {}", bark_id);

    let store = state.db.subscriptions();
    match store.delete_subscription(&bark_id) {
        Ok(_) => {
            tracing::info!("取消订阅成功 - Bark ID: {}", bark_id);
            (
                StatusCode::OK,
                Json(ApiResponse::<()>::success("已取消订阅", None)),
            )
        }
        Err(e) => {
            tracing::error!("取消订阅失败 - Bark ID: {}, 错误: {:?}", bark_id, e);
            (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()>::error(format!("取消订阅失败: {}", e))),
            )
        }
    }
}

/// 订阅成功响应
#[derive(Serialize)]
pub struct SubscribeResponse {
    pub bark_id: String,
    pub latitude: f64,
    pub longitude: f64,
    pub min_intensity: u8,
    pub bark_api_url: String,
    pub passive_max: u8,
    pub active_max: u8,
    pub created_at: i64,
}

impl From<Subscription> for SubscribeResponse {
    fn from(sub: Subscription) -> Self {
        Self {
            bark_id: sub.bark_id,
            latitude: sub.latitude,
            longitude: sub.longitude,
            min_intensity: sub.min_intensity,
            bark_api_url: sub.bark_api_url,
            passive_max: sub.passive_max,
            active_max: sub.active_max,
            created_at: sub.created_at,
        }
    }
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub total_subscriptions: usize,
}

/// 获取统计信息
pub async fn stats_handler(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.db.subscriptions();
    match store.get_total_count() {
        Ok(count) => (
            StatusCode::OK,
            Json(ApiResponse::success(
                "统计成功",
                Some(StatsResponse {
                    total_subscriptions: count,
                }),
            )),
        ),
        Err(e) => {
            tracing::error!("获取统计失败: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<StatsResponse>::error(format!(
                    "获取统计失败: {}",
                    e
                ))),
            )
        }
    }
}

/// 获取单个订阅信息
pub async fn get_subscription_handler(
    State(state): State<AppState>,
    Path(bark_id): Path<String>,
) -> impl IntoResponse {
    let store = state.db.subscriptions();
    match store.get_subscription(&bark_id) {
        Ok(sub) => (
            StatusCode::OK,
            Json(ApiResponse::success("ok", Some(SubscribeResponse::from(sub)))),
        ),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<SubscribeResponse>::error("订阅不存在")),
        ),
    }
}

/// 获取缓存的最近地震数据
pub async fn test_earthquake_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let cache = state.latest_earthquake.lock().unwrap();
    match cache.as_ref() {
        Some(eq) => (
            StatusCode::OK,
            Json(ApiResponse::success("ok", Some(eq.clone()))),
        ),
        None => (
            StatusCode::OK,
            Json(ApiResponse::<CachedEarthquake>::error("暂无地震数据")),
        ),
    }
}

/// 发送测试通知
pub async fn test_notify_handler(
    State(state): State<AppState>,
    Json(payload): Json<TestNotifyRequest>,
) -> impl IntoResponse {
    // 查订阅获取 bark_api_url 和级别设置
    let store = state.db.subscriptions();
    let sub = match store.get_subscription(&payload.bark_id) {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()>::error("订阅不存在")),
            );
        }
    };

    // 计算距离和预估烈度（复用正式算法）
    let dist = distance::vincenty_distance(
        payload.epicenter_lat,
        payload.epicenter_lon,
        sub.latitude,
        sub.longitude,
    )
    .unwrap_or(0.0);
    let estimated_intensity = intensity::estimate_intensity(payload.magnitude, dist);

    // 通知级别判定（与 bark_notifier.rs 逻辑一致）
    let level_params = if estimated_intensity <= sub.passive_max {
        "level=passive"
    } else if estimated_intensity <= sub.active_max {
        "level=active&volume=5"
    } else {
        "level=critical&volume=10&call=1"
    };

    // 构造消息
    let arrival_secs = (dist / 3.5).round() as u64;
    let p_wave_secs = (dist / 6.0).round() as u64;

    let title = format!("地震预警测试 {}秒后到达", arrival_secs);
    let subtitle = format!(
        "M{:.1} 预计烈度{} 距{:.0}km",
        payload.magnitude, estimated_intensity, dist
    );
    let body = format!(
        "[测试] 这是一条模拟预警，不是真实地震。\n\
         发震: {}\n\
         地点: {}\n\
         震源: {:.3}, {:.3} 深度{:.0}km\n\
         距离: 震中{:.0}km 震源{:.0}km\n\
         预计: P波+{}秒 S波+{}秒 烈度{}\n\
         震级: M{:.1} 最大烈度{:.1}\n\
         来源: test_eew 测试",
        payload.origin_time,
        payload.hypocenter,
        payload.epicenter_lat,
        payload.epicenter_lon,
        payload.depth,
        dist,
        dist,
        p_wave_secs,
        arrival_secs,
        estimated_intensity,
        payload.magnitude,
        payload.max_intensity,
    );

    // 发送 Bark 通知
    let base_url = sub.bark_api_url.trim_end_matches('/');
    let bark_url = format!(
        "{}/{}/{}/{}/{}?group=地震预警测试&{}",
        base_url,
        urlencoding::encode(&sub.bark_id),
        urlencoding::encode(&title),
        urlencoding::encode(&subtitle),
        urlencoding::encode(&body),
        level_params,
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();

    match client.get(&bark_url).send().await {
        Ok(resp) if resp.status().is_success() => (
            StatusCode::OK,
            Json(ApiResponse::success("测试通知已发送", None::<()>)),
        ),
        Ok(resp) => (
            StatusCode::BAD_GATEWAY,
            Json(ApiResponse::<()>::error(format!(
                "Bark 服务返回错误: {}",
                resp.status()
            ))),
        ),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(ApiResponse::<()>::error(format!("发送失败: {}", e))),
        ),
    }
}

/// 健康检查
pub async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(ApiResponse::<()>::success("OK", None)))
}
