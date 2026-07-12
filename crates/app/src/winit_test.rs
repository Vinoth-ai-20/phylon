//! Scratch coverage for matching `winit`'s trackpad gesture event variants
//! (pinch/pan/magnify). Not declared as a module anywhere in `main.rs`'s
//! module list, so this file is not currently compiled as part of the
//! `app` crate — kept as a quick reference for the match arms these events
//! need if trackpad gesture handling is added to the real event loop in
//! `events.rs`.

/// Exercises the match arms needed to handle `winit`'s trackpad gesture
/// events. Not wired into the real event loop; see this file's module doc.
pub fn test(event: winit::event::WindowEvent) {
    match event {
        winit::event::WindowEvent::PinchGesture { delta, .. } => {}
        winit::event::WindowEvent::PanGesture { delta, phase, .. } => {}
        winit::event::WindowEvent::TouchpadMagnify { delta, phase, .. } => {}
        _ => {}
    }
}
