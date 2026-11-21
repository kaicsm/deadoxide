//! Desktop-only queue between the rendering thread and the world-generation thread.
//!
//! `zombie.iso.WorldStreamer.addJob` queues I/O/generation away from the main thread;
//! `zombie.iso.IsoChunkMap.updateInternal` incorporates a bounded amount of completed
//! work per update. This Rust runtime mirrors the scheduling boundary, using a
//! separate, thread-local WorldGen context because the Java-equivalent generator
//! deliberately contains non-Send Rc-backed Lua-derived data.
//!
//! No GPU resources or game window handles cross this channel. The render-side
//! scene remains fully usable for movement, doors and collision while jobs run.

use crate::scene::{load_streaming, Scene, StreamReport, StreamSnapshot, StreamSnapshotCache};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use zomboid_engine::zombie::iso::streaming_window::ChunkWindow;


enum Command {
    Move { x: f32, y: f32 },
    Door { x: f32, y: f32, z: i32 },
    Priority { wx: i32, wy: i32 },
}

pub enum Completion {
    Prepared(StreamSnapshot),
    Failed(String),
}

/// Native WorldStreamer: one worker owns its isolated loading context for its lifetime.
/// Requests are coalesced before each batch, so turning around or teleporting does not
/// force the worker to finish an obsolete destination's entire chunk window.
pub struct AsyncWorldStreamer {
    sender: Sender<Command>,
    receiver: Receiver<Completion>,
    last_requested: ChunkWindow,
    last_position: (f32, f32),
    last_urgent: Option<(i32, i32)>,
    pending: VecDeque<Completion>,
}

impl AsyncWorldStreamer {
    /// Spawn after the initial visible scene has loaded. Initialization of the worker's
    /// independent generation context happens *on* the worker, not during this call.
    pub fn spawn(
        root: PathBuf, save: Option<PathBuf>, player: (f32, f32),
        radius: i32, persist_worldgen: bool,
    ) -> Result<Self, String> {
        let window = ChunkWindow::for_player(player.0, player.1, radius)?;
        let (sender, request_rx) = mpsc::channel();
        // Java WorldStreamer uses a completion queue separate from its job queue.
        // Never stall WorldGen because the renderer has not polled this frame.
        let (completion_tx, receiver) = mpsc::channel();
        thread::Builder::new().name("World Streamer".to_owned())
            .spawn(move || run_worker(root, save, player, radius, persist_worldgen,
                request_rx, completion_tx))
            .map_err(|e| format!("starting World Streamer: {e}"))?;
        Ok(Self { sender, receiver, last_requested: window,
            last_position: player, last_urgent: None,
            pending: VecDeque::new() })
    }

    /// Nonblocking, deduplicated requests; the worker picks up the latest target.
    pub fn request(&mut self, x: f32, y: f32) -> Result<(), String> {
        let next = ChunkWindow::for_player(x, y, self.last_requested.radius)?;
        // WorldStreamer.ChunkComparator.init reads the player's current and
        // previous positions for each sorting pass, even inside one chunk.
        if next != self.last_requested { self.last_urgent = None; }
        if next != self.last_requested || self.last_position != (x, y) {
            self.sender.send(Command::Move { x, y })
                .map_err(|e| format!("world streaming thread stopped: {e}"))?;
            self.last_requested = next;
            self.last_position = (x, y);
        }
        Ok(())
    }

    /// Schedule a missing collision square ahead of a moving character. This
    /// does not make an unloaded square walkable: it only reorders pending work.
    /// Returns true if a new hint was placed on the queue (for rate-limited logs).
    pub fn prioritize_chunk(&mut self, wx: i32, wy: i32, player: (f32, f32)) -> Result<bool, String> {
        if self.last_urgent == Some((wx, wy)) { return Ok(false); }
        // Reactivate the worker if it already finished the current window.
        self.sender.send(Command::Move { x: player.0, y: player.1 })
            .map_err(|e| format!("world streaming thread stopped: {e}"))?;
        self.sender.send(Command::Priority { wx, wy })
            .map_err(|e| format!("world streaming thread stopped: {e}"))?;
        self.last_urgent = Some((wx, wy));
        Ok(true)
    }

    pub fn toggle_door(&self, x: f32, y: f32, z: i32) -> Result<(), String> {
        self.sender.send(Command::Door { x, y, z })
            .map_err(|e| format!("world streaming thread stopped: {e}"))
    }

    /// IsoChunkMap.updateInternal: `count = 1 + ready * 3 / chunkGridWidth`.
    /// `WorldStreamer.threadLoop` prepares one job at a time independently;
    /// the main thread consumes a queue-sized budget of completed jobs per frame.
    /// Preserve FIFO order for stateful WorldGen/atlas deltas.
    pub fn poll(&mut self) -> Vec<Completion> {
        loop {
            match self.receiver.try_recv() {
                Ok(value) => self.pending.push_back(value),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if self.pending.is_empty() {
                        self.pending.push_back(Completion::Failed(
                            "World Streamer disconnected".to_owned()));
                    }
                    break;
                }
            }
        }
        if self.pending.is_empty() { return Vec::new(); }
        let width = (2 * self.last_requested.radius + 1) as usize;
        let budget = (1 + self.pending.len().saturating_mul(3) / width).min(self.pending.len());
        self.pending.drain(..budget).collect()
    }

}

fn publish(scene: &mut Scene, cache: &mut StreamSnapshotCache,
           tx: &Sender<Completion>, report: StreamReport,
           include_atlas: bool) -> bool {
    let start = std::time::Instant::now();
    let response = scene.stream_snapshot(cache, report, include_atlas)
        .map(Completion::Prepared)
        .unwrap_or_else(|e| Completion::Failed(format!("stream snapshot: {e}")));
    log::debug!("World Streamer snapshot prepared in {:.1}ms",
        start.elapsed().as_secs_f64() * 1000.0);
    tx.send(response).is_ok()
}

fn handle_command(scene: &mut Scene, cache: &mut StreamSnapshotCache,
                  command: Command, tx: &Sender<Completion>,
                  target: &mut Option<(f32, f32)>,
                  previous: &mut (f32, f32),
                  priority: &mut Option<(i32, i32)>) -> bool {
    match command {
        Command::Move { x, y } => {
            if let Some((old_x, old_y)) = *target { *previous = (old_x, old_y); }
            *target = Some((x, y)); true
        }
        Command::Priority { wx, wy } => {
            *priority = Some((wx, wy));
            true
        }
        Command::Door { x, y, z } => {
            if scene.toggle_door_near_unrebuilt(x, y, z) {
                publish(scene, cache, tx, StreamReport::default(), false)
            } else { true }
        }
    }
}

fn run_worker(
    root: PathBuf, save: Option<PathBuf>, player: (f32, f32),
    radius: i32, persist_worldgen: bool,
    requests: Receiver<Command>, completions: Sender<Completion>,
) {
    // The original WorldStreamer keeps a persistent loader and RNG. Do not move the
    // Rc-backed WorldGenRuntime through a channel or reinitialize it per job.
    let started = std::time::Instant::now();
    let mut scene = match load_streaming(&root, save.as_deref(), player.0, player.1,
        radius, persist_worldgen) {
        Ok(scene) => scene,
        Err(e) => {
            let _ = completions.send(Completion::Failed(format!("World Streamer initialization: {e}")));
            return;
        }
    };
    log::info!("World Streamer ready in {:.2}s", started.elapsed().as_secs_f32());
    // The two independently constructed initial atlases need not have the same
    // shelf-packing ties. Publish the worker's atlas before accepting its UVs.
    let mut cache = StreamSnapshotCache::default();
    if !publish(&mut scene, &mut cache, &completions, StreamReport {
        atlas_changed: true, ..Default::default()
    }, true) { return; }

    let mut target: Option<(f32, f32)> = None;
    let mut previous = player;
    let mut priority = None;
    loop {
        if target.is_none() {
            let Ok(cmd) = requests.recv() else { return; };
            if !handle_command(&mut scene, &mut cache, cmd, &completions,
                &mut target, &mut previous, &mut priority) { return; }
        }
        // Preserve door events and the latest movement request. Old destinations
        // are abandoned at batch boundaries rather than filling an unbounded job queue.
        while let Ok(cmd) = requests.try_recv() {
            if !handle_command(&mut scene, &mut cache, cmd, &completions,
                &mut target, &mut previous, &mut priority) { return; }
        }
        let Some((x, y)) = target else { continue; };
        let batch_start = std::time::Instant::now();
        // Like WorldStreamer.threadLoop, process one pending chunk per iteration.
        // Rendering integrates a variable number of completed chunks independently.
        match scene.stream_around_budget_projected_priority(
            x, y, previous.0, previous.1, 1, priority,
        ) {
            Ok(Some(report)) => {
                if priority.is_some_and(|(wx, wy)| scene.stream_chunk_resident(wx, wy)) {
                    priority = None;
                }
                log::debug!("World Streamer prepared {} chunks in {:.1}ms",
                    report.loaded, batch_start.elapsed().as_secs_f64() * 1000.0);
                if !publish(&mut scene, &mut cache, &completions, report, report.atlas_changed) { return; }
            }
            Ok(None) => { target = None; priority = None; },
            Err(e) => {
                if completions.send(Completion::Failed(format!("World Streamer {x},{y}: {e}"))).is_err() {
                    return;
                }
                target = None;
            }
        }
    }
}


#[cfg(test)]
mod scheduling_tests {
    #[test]
    fn java_update_internal_ready_queue_formula() {
        // zombie.iso.IsoChunkMap.updateInternal: if (count != 0)
        // count = 1 + count * 3 / chunkGridWidth.
        let java_budget = |ready: usize, width: usize| {
            if ready == 0 { 0 } else { (1 + ready * 3 / width).min(ready) }
        };
        assert_eq!(java_budget(0, 9), 0);
        assert_eq!(java_budget(1, 9), 1);
        assert_eq!(java_budget(9, 9), 4);
        assert_eq!(java_budget(18, 9), 7);
        assert_eq!(java_budget(25, 5), 16);
    }
}
