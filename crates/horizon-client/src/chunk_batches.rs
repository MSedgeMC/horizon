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

#[cfg(test)]
mod tests {
    use super::*;

    /// A calculator fed `samples` batches of `size` chunks, `nanos_per_chunk` each.
    fn after(samples: usize, size: i32, nanos_per_chunk: u64) -> ChunkBatchSizeCalculator {
        let mut c = ChunkBatchSizeCalculator::new();
        for _ in 0..samples {
            c.on_batch_finished_after(size, nanos_per_chunk * size as u64);
        }
        c
    }

    /// `ChunkBatchSizeCalculator.<init>`: 2 ms a chunk, so a fresh client asks for
    /// 3.5 before it has seen a batch.
    #[test]
    fn a_fresh_client_asks_for_the_starting_rate() {
        let c = ChunkBatchSizeCalculator::new();
        assert_eq!(c.desired_chunks_per_tick(), 3.5);
        assert_eq!(c.old_samples_weight, 1);
    }

    /// The first sample is halved with the starting average: 2 ms and 1 ms a
    /// chunk leave 1.5 ms, and the answer rises from 3.5 to 4.67 chunks a tick.
    #[test]
    fn a_first_batch_moves_the_average_halfway() {
        let c = after(1, 64, 1_000_000);
        assert_eq!(c.old_samples_weight, 2);
        assert!((c.aggregated_nanos_per_chunk - 1_500_000.0).abs() < 1e-6);
        assert!((c.desired_chunks_per_tick() - (7.0e6f64 / 1.5e6) as f32).abs() < 1e-6);
    }

    /// One sample may only move the average to a third of the way down or three
    /// times the way up (`CLAMP_COEFFICIENT`), so a stall (a section rebuild, a
    /// resource reload) or one lucky batch does not swing the answer.
    #[test]
    fn one_slow_or_fast_batch_is_clamped_before_it_counts() {
        // 100 ms a chunk against 2 ms: clamped to 6 ms, averaged to (2 + 6) / 2.
        let slow = after(1, 10, 100_000_000);
        assert!((slow.aggregated_nanos_per_chunk - 4_000_000.0).abs() < 1e-6);
        // 0.1 ms against 2 ms: clamped to 2 / 3 ms, averaged to (2 + 2/3) / 2.
        let fast = after(1, 10, 100_000);
        assert!((fast.aggregated_nanos_per_chunk - (2_000_000.0 * 2.0 / 3.0)).abs() < 1e-6);
    }

    /// A batch that carried no chunks (or a negative size, which the wire allows)
    /// is not a sample at all: neither the average nor the weight moves.
    #[test]
    fn an_empty_batch_is_no_sample() {
        let mut c = ChunkBatchSizeCalculator::new();
        c.on_batch_finished_after(0, 900_000_000);
        c.on_batch_finished_after(-4, 900_000_000);
        assert_eq!(c.aggregated_nanos_per_chunk, 2_000_000.0);
        assert_eq!(c.old_samples_weight, 1);
        assert_eq!(c.desired_chunks_per_tick(), 3.5);
    }

    /// The running average follows a steady rate and stops weighting new samples
    /// more than 49 old ones, so a long session still adapts.
    #[test]
    fn the_average_follows_a_steady_rate_and_caps_its_weight() {
        let c = after(500, 64, 1_000_000);
        assert_eq!(c.old_samples_weight, MAX_OLD_SAMPLES_WEIGHT);
        assert!(
            (c.aggregated_nanos_per_chunk - 1_000_000.0).abs() < 1_000.0,
            "{} ns a chunk",
            c.aggregated_nanos_per_chunk
        );
        assert!((c.desired_chunks_per_tick() - 7.0).abs() < 0.01);
    }

    /// The batch's clock starts at `on_batch_start`, and `on_batch_finished`
    /// measures the batch itself: a zero-sized batch after a real wait is still
    /// no sample.
    #[test]
    fn the_clock_starts_at_the_batch_start() {
        let mut c = ChunkBatchSizeCalculator::new();
        c.on_batch_start();
        c.on_batch_finished(0);
        assert_eq!(c.aggregated_nanos_per_chunk, 2_000_000.0);
        c.on_batch_start();
        c.on_batch_finished(-1);
        assert_eq!(c.desired_chunks_per_tick(), 3.5);
    }
}
