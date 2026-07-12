//! Screenshot/recording capture — reads the rendered swapchain texture back
//! to the CPU and encodes it as a PNG (screenshot) or accumulates frames for
//! a GIF (recording).
//!
//! The GPU readback mirrors the existing blocking-poll idiom already used in
//! this codebase for the timestamp-query readback (`render.rs`'s
//! `slice.map_async(...); device.poll(Maintain::Wait);` pattern) and the
//! buffer-to-buffer readback in `gpu::physics_pipeline` — a one-shot/throttled
//! UI-triggered readback doesn't need the async-channel-across-frames
//! complexity physics uses for its every-tick readback.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};

/// Accumulated state for an in-progress recording.
pub(crate) struct RecordingState {
    /// Captured frames, in order. Capped at `MAX_FRAMES` by the caller.
    pub frames: Vec<image::RgbaImage>,
    /// Wall-clock time of the last captured frame, used to throttle capture
    /// to `CAPTURE_INTERVAL` regardless of render framerate or sim speed.
    pub last_capture: Instant,
}

/// Real-time interval between captured recording frames (10 fps) — recording
/// reflects wall-clock playback smoothness, not simulation ticks.
pub(crate) const CAPTURE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

/// Hard cap on recorded frames (30s at 10fps) so a forgotten recording can't
/// grow the frame buffer unboundedly.
pub(crate) const MAX_RECORDING_FRAMES: usize = 300;

impl RecordingState {
    pub(crate) fn new() -> Self {
        Self {
            frames: Vec::new(),
            // Force an immediate capture on the very next frame.
            last_capture: Instant::now() - CAPTURE_INTERVAL,
        }
    }
}

/// Reads `texture` back to the CPU as an RGBA image, blocking until the GPU
/// copy completes. `format` must be a 4-byte-per-pixel format (the native
/// swapchain is `Bgra8UnormSrgb`/`Rgba8UnormSrgb`) — the B/R channels are
/// swapped for BGRA formats so the result is always RGBA.
pub(crate) fn read_texture_to_image(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> Option<image::RgbaImage> {
    let unpadded_bytes_per_row = width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

    let buffer_size = (padded_bytes_per_row * height) as wgpu::BufferAddress;
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("CaptureStagingBuffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("CaptureEncoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &staging_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let slice = staging_buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);

    let data = slice.get_mapped_range();
    let is_bgra = matches!(
        format,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    );

    // Strip row padding and (if needed) swap B/R, producing tightly-packed RGBA.
    let mut pixels = Vec::with_capacity((unpadded_bytes_per_row * height) as usize);
    for row in 0..height {
        let start = (row * padded_bytes_per_row) as usize;
        let row_bytes = &data[start..start + unpadded_bytes_per_row as usize];
        if is_bgra {
            for px in row_bytes.chunks_exact(4) {
                pixels.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
            }
        } else {
            pixels.extend_from_slice(row_bytes);
        }
    }
    drop(data);
    staging_buffer.unmap();

    image::RgbaImage::from_raw(width, height, pixels)
}

/// Saves `image` as a timestamped PNG under `./screenshots/`, creating the
/// directory if needed. Returns the path written.
pub(crate) fn save_screenshot(image: &image::RgbaImage) -> Result<PathBuf> {
    let dir = PathBuf::from("screenshots");
    std::fs::create_dir_all(&dir).context("creating ./screenshots directory")?;
    let path = dir.join(format!("phylon_{}.png", chrono_timestamp()));
    image
        .save(&path)
        .context("encoding/writing screenshot PNG")?;
    Ok(path)
}

/// Crops `image` to `(x, y, width, height)` (physical pixels, already
/// clamped to the image bounds by the caller) and saves it as a timestamped
/// PNG under `./screenshots/` — same destination/naming as
/// `save_screenshot`, since a chart export is just a narrower crop of the
/// same whole-window capture, not a separate artifact type. Returns the path
/// written.
pub(crate) fn save_chart_png(
    image: &image::RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<PathBuf> {
    let cropped = image::imageops::crop_imm(image, x, y, width, height).to_image();
    let dir = PathBuf::from("screenshots");
    std::fs::create_dir_all(&dir).context("creating ./screenshots directory")?;
    let path = dir.join(format!("phylon_chart_{}.png", chrono_timestamp()));
    cropped.save(&path).context("encoding/writing chart PNG")?;
    Ok(path)
}

/// Encodes `frames` as an animated GIF (one `CAPTURE_INTERVAL`-spaced frame
/// each) and saves it as a timestamped file under `./recordings/`, creating
/// the directory if needed. Returns the path written.
pub(crate) fn save_recording_gif(frames: &[image::RgbaImage]) -> Result<PathBuf> {
    let dir = PathBuf::from("recordings");
    std::fs::create_dir_all(&dir).context("creating ./recordings directory")?;
    let path = dir.join(format!("phylon_{}.gif", chrono_timestamp()));

    let file = std::fs::File::create(&path).context("creating recording GIF file")?;
    let mut encoder = image::codecs::gif::GifEncoder::new_with_speed(file, 10);
    encoder
        .set_repeat(image::codecs::gif::Repeat::Infinite)
        .context("setting GIF repeat mode")?;

    let delay = image::Delay::from_saturating_duration(CAPTURE_INTERVAL);
    for frame in frames {
        encoder
            .encode_frame(image::Frame::from_parts(frame.clone(), 0, 0, delay))
            .context("encoding GIF frame")?;
    }

    Ok(path)
}

/// Encodes and saves `frames` as a GIF, then reports success/failure via a
/// toast — the one place both the manual `MenuAction::ToggleRecording` stop
/// (`events.rs`) and the auto-stop-at-cap path (`render.rs`) report the
/// outcome, so there's a single save/toast implementation instead of two.
pub(crate) fn finish_recording(frames: &[image::RgbaImage], ui: &mut ui::WorkbenchState) {
    let frame_count = frames.len();
    match save_recording_gif(frames) {
        Ok(path) => {
            ui.push_toast(
                format!(
                    "Saved recording ({frame_count} frames) to {}",
                    path.display()
                ),
                ui::ToastSeverity::Success,
                4.0,
            );
        }
        Err(e) => {
            tracing::error!("Failed to save recording: {e}");
            ui.push_toast(
                format!("Failed to save recording: {e}"),
                ui::ToastSeverity::Error,
                5.0,
            );
        }
    }
}

/// A filesystem-safe local timestamp (no external `chrono` dependency —
/// `SystemTime` + a manual civil-calendar conversion is enough for a unique,
/// human-readable filename suffix).
fn chrono_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let millis = now.subsec_millis();
    // Days since epoch -> proleptic Gregorian calendar (civil_from_days,
    // Howard Hinnant's algorithm) — avoids pulling in a date/time crate for
    // just a filename timestamp.
    let days = (secs / 86400) as i64;
    let rem_secs = secs % 86400;
    let (hh, mm, ss) = (rem_secs / 3600, (rem_secs % 3600) / 60, rem_secs % 60);

    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!(
        "{y:04}{m:02}{d:02}_{hh:02}{mm:02}{ss:02}_{millis:03}",
        d = d
    )
}
