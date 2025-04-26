use std::sync::Arc;

use num::Complex;
use winit::{dpi::PhysicalSize, event::ElementState, window::Window};

use crate::gpu::Wgpu;

pub(super) struct InnerApp {
    pub window: Arc<Window>,
    pub surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,

    pub render_with_gpu: bool,
    pub gpu: Wgpu,

    pub focused: bool,
    pub in_window: bool,
    pub left_mouse: ElementState,
    // The re, im coordinates of the screen center in the mandelbrot space.
    pub view_center_point: Complex<f32>,
    pub zoom: f32,
    pub zoom_step: f32,
}

impl InnerApp {
    pub fn new(event_loop: &winit::event_loop::ActiveEventLoop) -> Self {
        let window_attributes = Window::default_attributes()
            .with_title("Mandelbrot")
            .with_resizable(false)
            .with_inner_size(winit::dpi::LogicalSize::new(1024.0, 768.0));

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        // Initialize the softbuffer surface, used for drawing
        let context = softbuffer::Context::new(Arc::clone(&window)).unwrap();
        let surface = softbuffer::Surface::new(&context, Arc::clone(&window)).unwrap();

        let gpu = pollster::block_on(Wgpu::new());

        InnerApp {
            window,
            surface,
            render_with_gpu: true,
            gpu,
            focused: true,
            in_window: false,
            left_mouse: ElementState::Released,
            view_center_point: Complex::new(-0.5, 0.0),
            zoom: 1.0,
            zoom_step: 1.0,
        }
    }
}

pub fn center_to_start_conditions(
    view_center: &Complex<f32>,
    zoom: f32,
    window_resolution: &PhysicalSize<u32>,
) -> (Complex<f32>, PhysicalSize<f32>) {
    // We would like to have the whole mandelbrot set in view right from the start.
    // On the imaginary axis it is about 2.3 units tall.
    // Based on that and the physical resolution of the window the view into
    // the mandelbrot space is scaled appropriately.
    let view_height = 2.3 * (1.0 / zoom);
    let view_width =
        (window_resolution.width as f32 / window_resolution.height as f32) * view_height;
    let view_resolution = PhysicalSize::new(view_width, view_height);

    let top_left = Complex::new(
        view_center.re - (view_width / 2.0),
        view_center.im + (view_height / 2.0),
    );

    (top_left, view_resolution)
}
