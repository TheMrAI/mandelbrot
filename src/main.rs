use std::{borrow::Cow, num::NonZeroU32, sync::Arc};

use num::Complex;
use wgpu::{
    Adapter, BindGroup, BindGroupEntry, BufferBinding, BufferUsages, Device, Queue, RenderPipeline,
    Surface,
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, ElementState, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::PhysicalKey,
    window::Window,
};

mod cpu;
use cpu::Cpu;

#[derive(Default)]
struct App {
    app: Option<InnerApp>,
}

struct InnerApp {
    pub window: Arc<Window>,

    pub render_with_gpu: bool,
    pub gpu: Wgpu,
    pub cpu: Cpu,

    pub focused: bool,
    pub in_window: bool,
    pub left_mouse: ElementState,
    // The x, y coordinates of the screen center
    pub center_point: (f32, f32),
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
        let gpu = pollster::block_on(Wgpu::new(Arc::clone(&window)));
        let cpu = Cpu::new(Arc::clone(&window));

        InnerApp {
            window,
            render_with_gpu: true,
            gpu,
            cpu,
            focused: true,
            in_window: false,
            left_mouse: ElementState::Released,
            center_point: (-0.5, 0.0),
            zoom: 1.0,
            zoom_step: 1.0,
        }
    }
}

fn center_to_start_conditions(
    view_center: (f32, f32),
    zoom: f32,
    window_resolution: (u32, u32),
) -> ((f32, f32), (f32, f32)) {
    // We would like to have the whole mandelbrot set in view right from the start.
    // On the imaginary axis it is about 2.3 units tall.
    // Based on that and the physical resolution of the window the view into
    // the mandelbrot space is scaled appropriately.
    let view_height = 2.3 * (1.0 / zoom);
    let view_width = (window_resolution.0 as f32 / window_resolution.1 as f32) * view_height;
    let top_left = (
        view_center.0 - (view_width / 2.0),
        view_center.1 + (view_height / 2.0),
    );

    (top_left, (view_width, view_height))
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
                        let frame = app
                            .gpu
                            .surface
                            .get_current_texture()
                            .expect("Failed to acquire next swap-chain texture.");

                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());

                        // Adjusted physical resolution for the given dpi setting on a given screen.
                        let window_resolution = app.window.inner_size();

                        let (upper_left, view_resolution) = center_to_start_conditions(
                            app.center_point,
                            app.zoom,
                            (window_resolution.width, window_resolution.height),
                        );

                        app.gpu.queue.write_buffer(
                            &app.gpu.uniform_buffer,
                            0,
                            &[
                                upper_left.0,
                                upper_left.1,
                                view_resolution.0,
                                view_resolution.1,
                                window_resolution.width as f32,
                                window_resolution.height as f32,
                            ]
                            .iter()
                            .flat_map(|entry| entry.to_ne_bytes())
                            .collect::<Vec<u8>>(),
                        );

                        let mut encoder = app.gpu.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("encoder"),
                            },
                        );
                        {
                            let mut render_pass =
                                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                    label: Some("render_pass"),
                                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                        view: &view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                            store: wgpu::StoreOp::Store,
                                        },
                                    })],
                                    depth_stencil_attachment: None,
                                    timestamp_writes: None,
                                    occlusion_query_set: None,
                                });
                            render_pass.set_pipeline(&app.gpu.render_pipeline);
                            render_pass.set_bind_group(0, &app.gpu.bind_group, &[]);
                            render_pass.draw(0..4, 0..1);
                        }
                        app.gpu.queue.submit(Some(encoder.finish()));

                        frame.present();

                    // Queue a RedrawRequested event.
                    //
                    // You only need to call this if you've determined that you need to redraw in
                    // applications which do not always need to. Applications that redraw continuously
                    // can render here instead.
                    // self.window.as_ref().unwrap().request_redraw();
                    } else {
                        let mut buffer = app.cpu.surface.buffer_mut().unwrap();

                        let window_resolution = app.window.inner_size();

                        let (top_left, view_resolution) = center_to_start_conditions(
                            (app.center_point.0, app.center_point.1),
                            app.zoom,
                            (window_resolution.width, window_resolution.height),
                        );
                        let upper_left = Complex {
                            re: top_left.0,
                            im: top_left.1,
                        };

                        cpu::render(
                            &mut buffer,
                            upper_left,
                            view_resolution,
                            (window_resolution.width, window_resolution.height),
                        );

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
            WindowEvent::Resized(inner_size) => {
                // Recreate the surface texture according to the new inner physical resolution.
                if let Some(app) = self.app.as_mut() {
                    if app.render_with_gpu {
                        let config = app
                            .gpu
                            .surface
                            .get_default_config(
                                &app.gpu.adapter,
                                inner_size.width,
                                inner_size.height,
                            )
                            .unwrap();
                        app.gpu.surface.configure(&app.gpu.device, &config);
                    } else {
                        let window_resolution = app.window.inner_size();
                        // TODO: handle softbuffer error
                        let _ = app.cpu.surface.resize(
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
                        app.center_point = (
                            app.center_point.0 + x_delta,
                            // invert y axis movement
                            app.center_point.1 - y_delta,
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
                                    app.center_point = (-0.5, 0.0);
                                    app.zoom = 1.0;
                                    app.window.request_redraw();
                                }
                            }
                            PhysicalKey::Code(winit::keyboard::KeyCode::KeyG) => {
                                if raw_key_event.state == ElementState::Released {
                                    app.render_with_gpu = true;

                                    let window_resolution = app.window.inner_size();
                                    let config = app
                                        .gpu
                                        .surface
                                        .get_default_config(
                                            &app.gpu.adapter,
                                            window_resolution.width,
                                            window_resolution.height,
                                        )
                                        .unwrap();
                                    app.gpu.surface.configure(&app.gpu.device, &config);

                                    app.window.request_redraw();
                                }
                            }
                            PhysicalKey::Code(winit::keyboard::KeyCode::KeyC) => {
                                if raw_key_event.state == ElementState::Released {
                                    app.render_with_gpu = false;

                                    let window_resolution = app.window.inner_size();
                                    // TODO: handle softbuffer error
                                    let _ = app.cpu.surface.resize(
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

struct Wgpu {
    pub adapter: Adapter,
    pub surface: Surface<'static>,
    pub device: Device,
    pub queue: Queue,
    pub bind_group: BindGroup,
    pub uniform_buffer: wgpu::Buffer,
    pub render_pipeline: RenderPipeline,
}

impl Wgpu {
    pub async fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::default();
        let window_size = window.inner_size();
        let surface = instance.create_surface(window).unwrap();
        // Request an adapter that can support our surface
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        // Create logical device and command queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .expect("Failed to create device");
        println!("Prepared device: {:?}", device);

        // Configure surface
        let config = surface
            .get_default_config(&adapter, window_size.width, window_size.height)
            .unwrap();
        surface.configure(&device, &config);

        // Load the shaders
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        // Uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("settings_uniform"),
            size: 6 * size_of::<f32>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bind group"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Settings"),
            layout: &bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: None, // use whole buffer
                }),
            }],
        });

        // Pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render_pipeline_descriptor"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(swapchain_format.into())],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None, // on some platforms it may be good to use such a cache to reduce shader compilation times, otherwise it is handled by most
        });

        Wgpu {
            adapter,
            surface,
            device,
            queue,
            bind_group,
            uniform_buffer,
            render_pipeline,
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
