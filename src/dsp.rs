use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;

pub const WAVEFORM_LEN: usize = 512;

/// Lock-free ring buffer for sharing waveform data between the audio and GUI threads.
/// Uses AtomicU32 (storing f32 bit patterns) so neither side ever blocks.
pub struct WaveformBuffer {
    data: Vec<AtomicU32>,
    write_pos: AtomicUsize,
}

// Safety: all fields are atomic, so concurrent access is safe.
unsafe impl Sync for WaveformBuffer {}

impl WaveformBuffer {
    pub fn new() -> Arc<Self> {
        let data = (0..WAVEFORM_LEN).map(|_| AtomicU32::new(0)).collect();
        Arc::new(Self {
            data,
            write_pos: AtomicUsize::new(0),
        })
    }

    /// Push a single sample. Called from the audio thread — fully lock-free.
    pub fn push(&self, sample: f32) {
        let pos = self.write_pos.load(Ordering::Relaxed);
        self.data[pos].store(sample.to_bits(), Ordering::Relaxed);
        self.write_pos.store((pos + 1) % WAVEFORM_LEN, Ordering::Relaxed);
    }

    /// Read a time-ordered snapshot (oldest first). Called from the GUI thread.
    pub fn read_snapshot(&self, out: &mut [f32; WAVEFORM_LEN]) {
        let wp = self.write_pos.load(Ordering::Relaxed);
        for i in 0..WAVEFORM_LEN {
            let idx = (wp + i) % WAVEFORM_LEN;
            out[i] = f32::from_bits(self.data[idx].load(Ordering::Relaxed));
        }
    }

    /// Zero out all data and reset write position. Called from `Plugin::reset()`.
    pub fn clear(&self) {
        for slot in &self.data {
            slot.store(0, Ordering::Relaxed);
        }
        self.write_pos.store(0, Ordering::Relaxed);
    }
}
