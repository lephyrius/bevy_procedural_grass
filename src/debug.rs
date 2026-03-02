use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderDebugSnapshot {
    pub queued_entities: u64,
    pub queued_chunk_handles: u64,
    pub pipeline_queued: u64,
    pub pipeline_creating: u64,
    pub pipeline_ready: u64,
    pub pipeline_pending: u64,
    pub pipeline_error: u64,
    pub set_grass_calls: u64,
    pub set_grass_missing: u64,
    pub set_wind_calls: u64,
    pub draw_calls: u64,
    pub drawn_chunks: u64,
    pub drawn_instances: u64,
    pub missing_chunk_buffers: u64,
}

static QUEUED_ENTITIES: AtomicU64 = AtomicU64::new(0);
static QUEUED_CHUNK_HANDLES: AtomicU64 = AtomicU64::new(0);
static PIPELINE_QUEUED: AtomicU64 = AtomicU64::new(0);
static PIPELINE_CREATING: AtomicU64 = AtomicU64::new(0);
static PIPELINE_READY: AtomicU64 = AtomicU64::new(0);
static PIPELINE_PENDING: AtomicU64 = AtomicU64::new(0);
static PIPELINE_ERROR: AtomicU64 = AtomicU64::new(0);
static SET_GRASS_CALLS: AtomicU64 = AtomicU64::new(0);
static SET_GRASS_MISSING: AtomicU64 = AtomicU64::new(0);
static SET_WIND_CALLS: AtomicU64 = AtomicU64::new(0);
static DRAW_CALLS: AtomicU64 = AtomicU64::new(0);
static DRAWN_CHUNKS: AtomicU64 = AtomicU64::new(0);
static DRAWN_INSTANCES: AtomicU64 = AtomicU64::new(0);
static MISSING_CHUNK_BUFFERS: AtomicU64 = AtomicU64::new(0);

pub fn begin_render_debug_frame() {
    QUEUED_ENTITIES.store(0, Ordering::Relaxed);
    QUEUED_CHUNK_HANDLES.store(0, Ordering::Relaxed);
    PIPELINE_QUEUED.store(0, Ordering::Relaxed);
    PIPELINE_CREATING.store(0, Ordering::Relaxed);
    PIPELINE_READY.store(0, Ordering::Relaxed);
    PIPELINE_PENDING.store(0, Ordering::Relaxed);
    PIPELINE_ERROR.store(0, Ordering::Relaxed);
    SET_GRASS_CALLS.store(0, Ordering::Relaxed);
    SET_GRASS_MISSING.store(0, Ordering::Relaxed);
    SET_WIND_CALLS.store(0, Ordering::Relaxed);
    DRAW_CALLS.store(0, Ordering::Relaxed);
    DRAWN_CHUNKS.store(0, Ordering::Relaxed);
    DRAWN_INSTANCES.store(0, Ordering::Relaxed);
    MISSING_CHUNK_BUFFERS.store(0, Ordering::Relaxed);
}

pub fn add_queued(entities: u64, chunk_handles: u64) {
    QUEUED_ENTITIES.fetch_add(entities, Ordering::Relaxed);
    QUEUED_CHUNK_HANDLES.fetch_add(chunk_handles, Ordering::Relaxed);
}

pub fn add_pipeline_status(ready: bool) {
    if ready {
        PIPELINE_READY.fetch_add(1, Ordering::Relaxed);
    } else {
        PIPELINE_PENDING.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn add_pipeline_queued() {
    PIPELINE_QUEUED.fetch_add(1, Ordering::Relaxed);
    PIPELINE_PENDING.fetch_add(1, Ordering::Relaxed);
}

pub fn add_pipeline_creating() {
    PIPELINE_CREATING.fetch_add(1, Ordering::Relaxed);
    PIPELINE_PENDING.fetch_add(1, Ordering::Relaxed);
}

pub fn add_pipeline_ready() {
    PIPELINE_READY.fetch_add(1, Ordering::Relaxed);
}

pub fn add_pipeline_error() {
    PIPELINE_ERROR.fetch_add(1, Ordering::Relaxed);
}

pub fn add_set_grass_call(missing: bool) {
    SET_GRASS_CALLS.fetch_add(1, Ordering::Relaxed);
    if missing {
        SET_GRASS_MISSING.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn add_set_wind_call() {
    SET_WIND_CALLS.fetch_add(1, Ordering::Relaxed);
}

pub fn add_draw_call() {
    DRAW_CALLS.fetch_add(1, Ordering::Relaxed);
}

pub fn add_drawn(chunks: u64, instances: u64) {
    DRAWN_CHUNKS.fetch_add(chunks, Ordering::Relaxed);
    DRAWN_INSTANCES.fetch_add(instances, Ordering::Relaxed);
}

pub fn add_missing_chunk_buffers(count: u64) {
    MISSING_CHUNK_BUFFERS.fetch_add(count, Ordering::Relaxed);
}

pub fn render_debug_snapshot() -> RenderDebugSnapshot {
    RenderDebugSnapshot {
        queued_entities: QUEUED_ENTITIES.load(Ordering::Relaxed),
        queued_chunk_handles: QUEUED_CHUNK_HANDLES.load(Ordering::Relaxed),
        pipeline_queued: PIPELINE_QUEUED.load(Ordering::Relaxed),
        pipeline_creating: PIPELINE_CREATING.load(Ordering::Relaxed),
        pipeline_ready: PIPELINE_READY.load(Ordering::Relaxed),
        pipeline_pending: PIPELINE_PENDING.load(Ordering::Relaxed),
        pipeline_error: PIPELINE_ERROR.load(Ordering::Relaxed),
        set_grass_calls: SET_GRASS_CALLS.load(Ordering::Relaxed),
        set_grass_missing: SET_GRASS_MISSING.load(Ordering::Relaxed),
        set_wind_calls: SET_WIND_CALLS.load(Ordering::Relaxed),
        draw_calls: DRAW_CALLS.load(Ordering::Relaxed),
        drawn_chunks: DRAWN_CHUNKS.load(Ordering::Relaxed),
        drawn_instances: DRAWN_INSTANCES.load(Ordering::Relaxed),
        missing_chunk_buffers: MISSING_CHUNK_BUFFERS.load(Ordering::Relaxed),
    }
}
