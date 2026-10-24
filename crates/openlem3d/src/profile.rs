//! `--profile`: where each frame's time goes, logged once a second. On the
//! CPU: the game's own work, recording the GPU's commands, waiting for the
//! window's next image, and handing the frame over. On the GPU, where the
//! driver has timestamps between commands: the clear, the scene's geometry,
//! its glow, presenting or stretching it, the UI and the copy onto the
//! window.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use l3d_engine::prelude::*;

/// The CPU's parts of a frame.
#[derive(Clone, Copy)]
pub enum Cpu {
    /// Input, simulation, UI and scene updates: everything before drawing.
    Game,
    /// Waiting for the window's next image.
    Acquire,
    /// Recording the GPU's commands.
    Encode,
    /// Submitting them and presenting.
    Present,
}

const CPU_NAMES: [&str; 4] = ["game", "acquire", "encode", "present"];
/// The GPU's timestamps, in the order the frame writes them: its start,
/// then the end of each part.
#[derive(Clone, Copy)]
pub enum Stamp {
    Start,
    Clear,
    Geometry,
    Bloom,
    Scene,
    Ui,
    Copy,
}

/// The GPU's parts of a frame, between consecutive timestamps.
const GPU_NAMES: [&str; 6] = ["clear", "geometry", "bloom", "present", "ui", "copy"];
const STAMPS: u32 = GPU_NAMES.len() as u32 + 1;
/// Readback buffers in flight, so that reading never waits for the GPU.
const RING: usize = 4;

/// Sums and worst cases since the last report.
#[derive(Default)]
struct Totals {
    frames: u32,
    /// Start to start of consecutive frames.
    interval: Duration,
    worst_interval: Duration,
    /// Intervals over 20 ms (a missed refresh at 60 Hz).
    slow: u32,
    cpu: [Duration; 4],
    worst_cpu: [Duration; 4],
    gpu_frames: u32,
    gpu: [f64; 6],
    worst_gpu: [f64; 6],
}

struct Readback {
    buffer: wgpu::Buffer,
    /// Copied into this frame and waiting to be mapped or read.
    pending: bool,
    mapped: Arc<AtomicBool>,
}

/// The GPU timestamps: one set written each frame, resolved and copied
/// into a free readback buffer.
struct GpuTimer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    ring: Vec<Readback>,
    /// The readback buffer this frame copies into, if one was free.
    current: Option<usize>,
    /// Nanoseconds per tick.
    period: f64,
}

#[derive(Default)]
pub struct Profile {
    pub on: bool,
    frame_start: Option<Instant>,
    mark: Option<Instant>,
    last_report: Option<Instant>,
    totals: Totals,
    gpu: Option<GpuTimer>,
}

/// The features the GPU timer needs.
pub const FEATURES: wgpu::Features =
    wgpu::Features::TIMESTAMP_QUERY.union(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS);

impl Profile {
    pub fn new(on: bool) -> Self {
        Profile { on, ..default() }
    }

    /// The start of a frame.
    pub fn begin_frame(&mut self) {
        if !self.on {
            return;
        }
        let now = Instant::now();
        if let Some(start) = self.frame_start {
            let t = &mut self.totals;
            let d = now - start;
            t.frames += 1;
            t.interval += d;
            t.worst_interval = t.worst_interval.max(d);
            t.slow += u32::from(d > Duration::from_millis(20));
        }
        self.frame_start = Some(now);
        self.mark = Some(now);
        let last = *self.last_report.get_or_insert(now);
        if now - last >= Duration::from_secs(1) {
            self.report();
            self.last_report = Some(now);
        }
    }

    /// The end of part `c` of the frame, which began at the last mark.
    pub fn cpu(&mut self, c: Cpu) {
        if !self.on {
            return;
        }
        let now = Instant::now();
        if let Some(mark) = self.mark.replace(now) {
            let i = c as usize;
            let t = &mut self.totals;
            t.cpu[i] += now - mark;
            t.worst_cpu[i] = t.worst_cpu[i].max(now - mark);
        }
    }

    /// Makes the GPU timer for `device`, if it has the features.
    pub fn start_gpu(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if !self.on || self.gpu.is_some() {
            return;
        }
        if !device.features().contains(FEATURES) {
            info!("profile: no GPU timestamps between commands on this device");
            return;
        }
        let size = STAMPS as u64 * 8;
        let buffer = |label, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        self.gpu = Some(GpuTimer {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("profile"),
                ty: wgpu::QueryType::Timestamp,
                count: STAMPS,
            }),
            resolve: buffer(
                "profile resolve",
                wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            ),
            ring: (0..RING)
                .map(|_| Readback {
                    buffer: buffer(
                        "profile readback",
                        wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    ),
                    pending: false,
                    mapped: Arc::new(AtomicBool::new(false)),
                })
                .collect(),
            current: None,
            period: queue.get_timestamp_period() as f64,
        });
    }

    /// Reads the timestamps of earlier frames that have come back, and
    /// picks a free buffer for this frame's.
    pub fn collect(&mut self, device: &wgpu::Device) {
        let Some(g) = &mut self.gpu else { return };
        let _ = device.poll(wgpu::PollType::Poll);
        let t = &mut self.totals;
        for r in &mut g.ring {
            if !(r.pending && r.mapped.load(Ordering::Acquire)) {
                continue;
            }
            {
                let bytes = r.buffer.get_mapped_range(..);
                let stamps: Vec<u64> = bytes
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|c| u64::from_le_bytes(*c))
                    .collect();
                // Some drivers (Apple's) write zeros between commands.
                if stamps.windows(2).all(|w| w[1] >= w[0])
                    && stamps[STAMPS as usize - 1] > stamps[0]
                {
                    t.gpu_frames += 1;
                    for (i, w) in stamps.windows(2).enumerate() {
                        let ms = (w[1] - w[0]) as f64 * g.period / 1e6;
                        t.gpu[i] += ms;
                        t.worst_gpu[i] = t.worst_gpu[i].max(ms);
                    }
                }
            }
            r.buffer.unmap();
            r.pending = false;
            r.mapped.store(false, Ordering::Release);
        }
        g.current = g.ring.iter().position(|r| !r.pending);
    }

    /// Writes timestamp `s`.
    pub fn stamp(&self, encoder: &mut wgpu::CommandEncoder, s: Stamp) {
        if let Some(g) = &self.gpu
            && g.current.is_some()
        {
            encoder.write_timestamp(&g.queries, s as u32);
        }
    }

    /// Resolves this frame's timestamps into its readback buffer.
    pub fn resolve(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let Some(g) = &mut self.gpu else { return };
        let Some(i) = g.current else { return };
        encoder.resolve_query_set(&g.queries, 0..STAMPS, &g.resolve, 0);
        encoder.copy_buffer_to_buffer(&g.resolve, 0, &g.ring[i].buffer, 0, STAMPS as u64 * 8);
        g.ring[i].pending = true;
    }

    /// Asks for this frame's readback buffer once its commands are submitted.
    pub fn submitted(&mut self) {
        let Some(g) = &mut self.gpu else { return };
        let Some(i) = g.current.take() else { return };
        let mapped = g.ring[i].mapped.clone();
        g.ring[i]
            .buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                if r.is_ok() {
                    mapped.store(true, Ordering::Release);
                }
            });
    }

    fn report(&mut self) {
        let t = std::mem::take(&mut self.totals);
        if t.frames == 0 {
            return;
        }
        let ms = |d: Duration| d.as_secs_f64() * 1e3;
        let n = t.frames as f64;
        let cpu: Vec<String> = (0..4)
            .map(|i| {
                format!(
                    "{} {:.1}/{:.1}",
                    CPU_NAMES[i],
                    ms(t.cpu[i]) / n,
                    ms(t.worst_cpu[i])
                )
            })
            .collect();
        let gpu: Vec<String> = (0..GPU_NAMES.len())
            .filter(|_| t.gpu_frames > 0)
            .map(|i| {
                format!(
                    "{} {:.1}/{:.1}",
                    GPU_NAMES[i],
                    t.gpu[i] / t.gpu_frames as f64,
                    t.worst_gpu[i]
                )
            })
            .collect();
        info!(
            "profile: {:.0} fps, frame {:.1}/{:.1} ms, {} over 20 ms | cpu (mean/worst ms) {} | gpu {}",
            n / t.interval.as_secs_f64(),
            ms(t.interval) / n,
            ms(t.worst_interval),
            t.slow,
            cpu.join(", "),
            if gpu.is_empty() {
                "no timings from this driver".into()
            } else {
                gpu.join(", ")
            },
        );
    }
}
