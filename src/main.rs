use std::{num::NonZeroU32, sync::Arc};

use num::Complex;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::PhysicalKey,
    window::Window,
};

mod cpu;
mod gpu;

use gpu::Wgpu;

#[derive(Default)]
struct App {
    app: Option<InnerApp>,
}

struct InnerApp {
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

fn center_to_start_conditions(
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

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        // The Window should be created in this call, because the winit documentation states that this
        // is the only point which they could guarantee proper initialization on all supported platforms.
        // And since WebGPU heavily relies on the Window object, this is where that should be initialized as well.
        self.app = Some(InnerApp::new(event_loop));
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId, // we only have one window
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                // Draw.
                if let Some(app) = self.app.as_mut() {
                    if app.render_with_gpu {
                        let mut buffer = app.surface.buffer_mut().unwrap();

                        // Adjusted physical resolution for the given dpi setting on a given screen.
                        let window_resolution = app.window.inner_size();

                        let (top_left, view_resolution) = center_to_start_conditions(
                            &app.view_center_point,
                            app.zoom,
                            &window_resolution,
                        );

                        app.gpu
                            .render(&mut buffer, top_left, &view_resolution, &window_resolution);

                        buffer.present().unwrap();
                    } else {
                        let mut buffer = app.surface.buffer_mut().unwrap();

                        let window_resolution = app.window.inner_size();

                        let (top_left, view_resolution) = center_to_start_conditions(
                            &app.view_center_point,
                            app.zoom,
                            &window_resolution,
                        );

                        cpu::render(&mut buffer, top_left, &view_resolution, &window_resolution);

                        buffer.present().unwrap();
                    }
                }
                // else nothing to do yet
            }
            WindowEvent::Focused(focused) => {
                if let Some(app) = self.app.as_mut() {
                    app.focused = focused;
                    if !focused {
                        // Make sure the mouse button is considered Released
                        // when the Window looses focus, as it is impossible to
                        // catch the release event when the user clicked off.
                        app.left_mouse = ElementState::Released;
                    }
                }
            }
            WindowEvent::CursorEntered { device_id: _ } => {
                if let Some(app) = self.app.as_mut() {
                    app.in_window = true;
                }
            }
            WindowEvent::CursorLeft { device_id: _ } => {
                if let Some(app) = self.app.as_mut() {
                    app.in_window = false;
                }
            }
            WindowEvent::Resized(window_resolution) => {
                // Recreate the surface texture according to the new inner physical resolution.
                if let Some(app) = self.app.as_mut() {
                    let _ = app.surface.resize(
                        NonZeroU32::new(window_resolution.width).unwrap(),
                        NonZeroU32::new(window_resolution.height).unwrap(),
                    );
                    if app.render_with_gpu {
                        // let config = app
                        //     .gpu
                        //     .surface
                        //     .get_default_config(
                        //         &app.gpu.adapter,
                        //         inner_size.width,
                        //         inner_size.height,
                        //     )
                        //     .unwrap();
                        // app.gpu.surface.configure(&app.gpu.device, &config);
                    } else {
                        let window_resolution = app.window.inner_size();
                        // TODO: handle softbuffer error
                        let _ = app.surface.resize(
                            NonZeroU32::new(window_resolution.width).unwrap(),
                            NonZeroU32::new(window_resolution.height).unwrap(),
                        );
                    }
                }
            }
            _ => (),
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        match event {
            DeviceEvent::MouseWheel { delta } => {
                if let Some(app) = self.app.as_mut() {
                    if app.focused && app.in_window {
                        println!("{:?} MouseWheel delta: {:?}", device_id, delta);
                        match delta {
                            winit::event::MouseScrollDelta::LineDelta(_, dy) => {
                                app.zoom_step += dy;
                                // Limit zoom_step between [1, ~130_000].
                                // Outside the ranges we will see only heavy pixelization
                                // or calculation errors.
                                app.zoom_step = app.zoom_step.clamp(1.0f32, 60f32);
                                // Using a decently aggressive function for mapping the zoom_step
                                // counter into actual zoom value.
                                // The *0.01 is meant to widen the curve, while the 0.99 ensures
                                // that using the initial zoom_step and zoom of 1.0, no jarring
                                // transition occurs.
                                app.zoom = app.zoom_step.powf(4.0) * 0.01 + 0.99;
                            }
                            _ => panic!("Interface not yet supported"),
                        }
                        app.window.request_redraw();
                    }
                }
            }
            DeviceEvent::MouseMotion { delta } => {
                if let Some(app) = self.app.as_mut() {
                    if app.focused && app.in_window && app.left_mouse == ElementState::Pressed {
                        println!("{:?} MouseMotion delta: {:?}", device_id, delta);
                        // Scale the panning movement on the current zoom level.
                        // The more zoomed in the view, the less should the camera pan
                        // on movement.
                        let x_delta = (delta.0 as f32 / 100.0) / app.zoom;
                        let y_delta = (delta.1 as f32 / 100.0) / app.zoom;
                        app.view_center_point = Complex::new(
                            app.view_center_point.re + x_delta,
                            // invert y axis movement
                            app.view_center_point.im - y_delta,
                        );
                        app.window.request_redraw();
                    }
                }
            }
            DeviceEvent::Button { button, state } => {
                if let Some(app) = self.app.as_mut() {
                    // Left mouse button
                    if button == 0 {
                        app.left_mouse = state;
                    }
                    println!("{:?} {:?}", button, state);
                }
            }
            DeviceEvent::Key(raw_key_event) => {
                if let Some(app) = self.app.as_mut() {
                    if app.focused && app.in_window {
                        // reset view
                        match raw_key_event.physical_key {
                            PhysicalKey::Code(winit::keyboard::KeyCode::KeyR) => {
                                if raw_key_event.state == ElementState::Released {
                                    app.view_center_point = Complex::new(-0.5, 0.0);
                                    app.zoom = 1.0;
                                    app.window.request_redraw();
                                }
                            }
                            PhysicalKey::Code(winit::keyboard::KeyCode::KeyG) => {
                                if raw_key_event.state == ElementState::Released {
                                    app.render_with_gpu = true;

                                    // let window_resolution = app.window.inner_size();
                                    // let config = app
                                    //     .gpu
                                    //     .surface
                                    //     .get_default_config(
                                    //         &app.gpu.adapter,
                                    //         window_resolution.width,
                                    //         window_resolution.height,
                                    //     )
                                    //     .unwrap();
                                    // app.gpu.surface.configure(&app.gpu.device, &config);

                                    app.window.request_redraw();
                                }
                            }
                            PhysicalKey::Code(winit::keyboard::KeyCode::KeyC) => {
                                if raw_key_event.state == ElementState::Released {
                                    app.render_with_gpu = false;

                                    let window_resolution = app.window.inner_size();
                                    // TODO: handle softbuffer error
                                    let _ = app.surface.resize(
                                        NonZeroU32::new(window_resolution.width).unwrap(),
                                        NonZeroU32::new(window_resolution.height).unwrap(),
                                    );

                                    app.window.request_redraw();
                                }
                            }
                            _ => (), // do nothing
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
    // dispatched any events. This is ideal for games and similar applications.
    // event_loop.set_control_flow(ControlFlow::Poll);
    // ControlFlow::Wait pauses the event loop if no events are available to process.
    // This is ideal for non-game applications that only update in response to user
    // input, and uses significantly less power/CPU time than ControlFlow::Poll.
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    let _ = event_loop.run_app(&mut app);
}
