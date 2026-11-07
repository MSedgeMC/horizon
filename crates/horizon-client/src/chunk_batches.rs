//! `ChunkBatchSizeCalculator`: how many chunks per tick a client asks its server
//! for, from how long the batches it has received took to arrive.
//!
//! The server sends a run of chunks between a `ChunkBatchStart` and a
//! `ChunkBatchFinished(size)` and paces the next run by the answer
//! (`PlayerChunkSender.onChunkBatchReceivedByClient`). A client that always
//! answered the ceiling would have a server whose chunks take 40 ms each send
//! them as fast as it can encode them, growing the queue the client has to
//! read through before the chunks it stands in; one that always answered a
//! penny would crawl. Vanilla measures the last batch's time per chunk and
//! answers `7 ms / that`, with each sample clamped to a third and three times
//! the running average, so one stall or one lucky batch does not swing it.
use std::time::Instant;

/// The chunks per tick the answer aims at: 7 ms of arriving chunks a tick.
const BATCH_BUDGET_NANOS: f64 = 7_000_000.0;
/// `ChunkBatchSizeCalculator.MAX_OLD_SAMPLES_WEIGHT`: the running average's weight
/// stops growing here, so a long session still follows a change.
const MAX_OLD_SAMPLES_WEIGHT: u32 = 49;
/// How far one sample may move the average (`ChunkBatchSizeCalculator.CLAMP_COEFFICIENT`).
const CLAMP_COEFFICIENT: f64 = 3.0;
/// The average a fresh calculator starts from: 2 ms a chunk.
const INITIAL_NANOS_PER_CHUNK: f64 = 2_000_000.0;

#[derive(Debug)]
pub struct ChunkBatchSizeCalculator {
    aggregated_nanos_per_chunk: f64,
    old_samples_weight: u32,
    chunk_batch_start: Instant,
}
impl Default for ChunkBatchSizeCalculator {
    fn default() -> Self {
        Self::new()
    }
}
impl ChunkBatchSizeCalculator {
    pub fn new() -> Self {
        Self {
            aggregated_nanos_per_chunk: INITIAL_NANOS_PER_CHUNK,
            old_samples_weight: 1,
            chunk_batch_start: Instant::now(),
        }
    }
    /// `onBatchStart`: the next batch's clock starts here.
    pub fn on_batch_start(&mut self) {
        self.chunk_batch_start = Instant::now();
    }
    /// `onBatchFinished`: the batch's chunks took the time since the last
    /// [`on_batch_start`](Self::on_batch_start).
    pub fn on_batch_finished(&mut self, size: i32) {
        let elapsed = self.chunk_batch_start.elapsed();
        self.on_batch_finished_after(size, elapsed.as_nanos() as u64);
    }
    /// The same with the batch's time given (`on_batch_finished` measures it).
    fn on_batch_finished_after(&mut self, size: i32, elapsed_nanos: u64) {
        if size <= 0 {
            return;
        }
        let per_chunk = elapsed_nanos as f64 / f64::from(size);
        let clamped = per_chunk.clamp(
            self.aggregated_nanos_per_chunk / CLAMP_COEFFICIENT,
            self.aggregated_nanos_per_chunk * CLAMP_COEFFICIENT,
        );
        let weight = f64::from(self.old_samples_weight);
        self.aggregated_nanos_per_chunk =
            (self.aggregated_nanos_per_chunk * weight + clamped) / (weight + 1.0);
        self.old_samples_weight = (self.old_samples_weight + 1).min(MAX_OLD_SAMPLES_WEIGHT);
    }
    /// `getDesiredChunksPerTick`: `ServerboundChunkBatchReceivedPacket`'s value.
    pub fn desired_chunks_per_tick(&self) -> f32 {
        (BATCH_BUDGET_NANOS / self.aggregated_nanos_per_chunk) as f32
    }
}
