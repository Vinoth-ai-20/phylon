pub fn test(event: winit::event::WindowEvent) {
    match event {
        winit::event::WindowEvent::PinchGesture { delta, .. } => {}
        winit::event::WindowEvent::PanGesture { delta, phase, .. } => {}
        winit::event::WindowEvent::TouchpadMagnify { delta, phase, .. } => {}
        _ => {}
    }
}
