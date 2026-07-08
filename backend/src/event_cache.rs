use crate::models::CachedEvent;
use std::sync::Arc;
use tokio::sync::RwLock;

const MAX_EVENTS: usize = 200;

#[derive(Clone)]
pub struct EventCache {
    events: Arc<RwLock<Vec<CachedEvent>>>,
}

impl EventCache {
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::with_capacity(MAX_EVENTS))),
        }
    }

    pub async fn push(&self, event: CachedEvent) {
        let mut events = self.events.write().await;
        if events.len() >= MAX_EVENTS {
            events.remove(0);
        }
        events.push(event);
    }

    pub async fn list(
        &self,
        source: Option<&str>,
        min_mag: f64,
        page: usize,
        page_size: usize,
    ) -> (Vec<CachedEvent>, usize) {
        let events = self.events.read().await;

        let filtered: Vec<&CachedEvent> = events
            .iter()
            .filter(|e| {
                if let Some(src) = source {
                    if e.source_type != src {
                        return false;
                    }
                }
                if min_mag > 0.0 && e.magnitude < min_mag {
                    return false;
                }
                true
            })
            .collect();

        let total = filtered.len();
        let total_pages = total.div_ceil(page_size);

        let start = (page.saturating_sub(1)) * page_size;
        let end = std::cmp::min(start + page_size, total);

        let page_events: Vec<CachedEvent> = if start < total {
            filtered[start..end].iter().map(|e| (*e).clone()).collect()
        } else {
            Vec::new()
        };

        (page_events, total_pages)
    }

    pub async fn get_by_id(&self, id: &str) -> Option<CachedEvent> {
        let events = self.events.read().await;
        events.iter().find(|e| e.id == id).cloned()
    }
}
