use std::{borrow::Cow, num::NonZeroU32, sync::Arc};

use num::Complex;
use wgpu::{Adapter, BindGroupEntry, BufferBinding, BufferUsages, Device, Queue};
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
                        // Adjusted physical resolution for the given dpi setting on a given screen.
                        let window_resolution = app.window.inner_size();

                        // PREPARE COMPUTE
                        // allocate local texture representation
                        let mut texture_data = vec![
                            0u8;
                            (window_resolution.width * window_resolution.height * 4)
                                as usize
                        ];
                        // Load the shaders
                        let shader =
                            app.gpu
                                .device
                                .create_shader_module(wgpu::ShaderModuleDescriptor {
                                    label: Some("shader"),
                                    source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                                        "shader.wgsl"
                                    ))),
                                });

                        // Storage texture for calculation output
                        let storage_texture =
                            app.gpu.device.create_texture(&wgpu::TextureDescriptor {
                                label: Some("mandelbrot result texture"),
                                size: wgpu::Extent3d {
                                    width: window_resolution.width,
                                    height: window_resolution.height,
                                    depth_or_array_layers: 1,
                                },
                                mip_level_count: 1,
                                sample_count: 1,
                                dimension: wgpu::TextureDimension::D2,
                                format: wgpu::TextureFormat::Rgba8Unorm,
                                usage: wgpu::TextureUsages::STORAGE_BINDING
                                    | wgpu::TextureUsages::COPY_SRC,
                                view_formats: &[],
                            });
                        // TODO why default?
                        let storage_texture_view =
                            storage_texture.create_view(&wgpu::TextureViewDescriptor::default());
                        let output_staging_buffer =
                            app.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                                label: Some("output staging buffer"),
                                size: size_of_val(&texture_data[..]) as u64,
                                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                                mapped_at_creation: false,
                            });

                        // Uniform buffer
                        let uniform_buffer =
                            app.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                                label: Some("settings_uniform"),
                                size: 6 * size_of::<f32>() as u64,
                                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                                mapped_at_creation: false,
                            });

                        let bind_group_layout = app.gpu.device.create_bind_group_layout(
                            &wgpu::BindGroupLayoutDescriptor {
                                label: Some("Bind group layout"),
                                entries: &[
                                    wgpu::BindGroupLayoutEntry {
                                        binding: 0,
                                        visibility: wgpu::ShaderStages::COMPUTE,
                                        ty: wgpu::BindingType::Buffer {
                                            ty: wgpu::BufferBindingType::Uniform,
                                            has_dynamic_offset: false,
                                            min_binding_size: None,
                                        },
                                        count: None,
                                    },
                                    wgpu::BindGroupLayoutEntry {
                                        binding: 1,
                                        visibility: wgpu::ShaderStages::COMPUTE,
                                        ty: wgpu::BindingType::StorageTexture {
                                            access: wgpu::StorageTextureAccess::WriteOnly,
                                            format: wgpu::TextureFormat::Rgba8Unorm,
                                            view_dimension: wgpu::TextureViewDimension::D2,
                                        },
                                        count: None,
                                    },
                                ],
                            },
                        );

                        // Create bind group
                        let bind_group =
                            app.gpu
                                .device
                                .create_bind_group(&wgpu::BindGroupDescriptor {
                                    label: Some("bind group"),
                                    layout: &bind_group_layout,
                                    entries: &[
                                        BindGroupEntry {
                                            binding: 0,
                                            resource: wgpu::BindingResource::Buffer(
                                                BufferBinding {
                                                    buffer: &uniform_buffer,
                                                    offset: 0,
                                                    size: None, // use whole buffer
                                                },
                                            ),
                                        },
                                        BindGroupEntry {
                                            binding: 1,
                                            resource: wgpu::BindingResource::TextureView(
                                                &storage_texture_view,
                                            ),
                                        },
                                    ],
                                });

                        // Pipeline
                        let pipeline_layout = app.gpu.device.create_pipeline_layout(
                            &wgpu::PipelineLayoutDescriptor {
                                label: Some("pipeline_layout"),
                                bind_group_layouts: &[&bind_group_layout],
                                push_constant_ranges: &[],
                            },
                        );
                        let compute_pipeline = app.gpu.device.create_compute_pipeline(
                            &wgpu::ComputePipelineDescriptor {
                                label: Some("mandelbrot compute pipeline"),
                                layout: Some(&pipeline_layout),
                                module: &shader,
                                entry_point: Some("main"),
                                compilation_options: Default::default(),
                                cache: None,
                            },
                        );

                        let (upper_left, view_resolution) = center_to_start_conditions(
                            app.center_point,
                            app.zoom,
                            (window_resolution.width, window_resolution.height),
                        );

                        app.gpu.queue.write_buffer(
                            &uniform_buffer,
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

                        let mut command_encoder = app.gpu.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("compute command encoder"),
                            },
                        );
                        {
                            // run computation command
                            let mut compute_pass =
                                command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                    label: Some("compute pass"),
                                    timestamp_writes: None,
                                });
                            compute_pass.set_bind_group(0, &bind_group, &[]);
                            compute_pass.set_pipeline(&compute_pipeline);
                            compute_pass.dispatch_workgroups(
                                window_resolution.width,
                                window_resolution.height,
                                1,
                            );
                        }

                        // download texture command
                        command_encoder.copy_texture_to_buffer(
                            wgpu::TexelCopyTextureInfoBase {
                                texture: &storage_texture,
                                mip_level: 0,
                                origin: wgpu::Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            wgpu::TexelCopyBufferInfoBase {
                                buffer: &output_staging_buffer,
                                layout: wgpu::TexelCopyBufferLayout {
                                    offset: 0,
                                    bytes_per_row: Some(window_resolution.width * 4),
                                    rows_per_image: Some(window_resolution.height),
                                },
                            },
                            wgpu::Extent3d {
                                width: window_resolution.width,
                                height: window_resolution.height,
                                depth_or_array_layers: 1,
                            },
                        );
                        app.gpu.queue.submit(Some(command_encoder.finish()));

                        let buffer_slice = output_staging_buffer.slice(..);
                        // TODO do you need to synchronize on the callback result or is it enough that your poll
                        // has returned? for the time being I think it should be enough, but more investigation is
                        // warranted
                        buffer_slice.map_async(wgpu::MapMode::Read, move |_| {});
                        app.gpu.device.poll(wgpu::PollType::Wait).unwrap();
                        {
                            let view = buffer_slice.get_mapped_range();
                            texture_data.copy_from_slice(&view[..]);
                        }
                        output_staging_buffer.unmap();

                        let mut buffer = app.cpu.surface.buffer_mut().unwrap();
                        // this is rather nasty
                        // softbuffer expects the value as ARGB while
                        // the texture is produced as RGBA
                        // TODO maybe we can do better?
                        for row in 0..window_resolution.height {
                            for column in 0..window_resolution.width {
                                let texture_column_width = window_resolution.width * 4;
                                let texture_index =
                                    ((row * texture_column_width) + column * 4) as usize;
                                let shifted = (texture_data[texture_index] as u32) << 16
                                    | (texture_data[texture_index + 1] as u32) << 8
                                    | (texture_data[texture_index + 2] as u32);
                                let pixel_index = (row * window_resolution.width + column) as usize;
                                buffer[pixel_index] = shifted;
                            }
                        }

                        buffer.present().unwrap();
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
            WindowEvent::Resized(window_resolution) => {
                // Recreate the surface texture according to the new inner physical resolution.
                if let Some(app) = self.app.as_mut() {
                    let _ = app.cpu.surface.resize(
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
    pub device: Device,
    pub queue: Queue,
}

impl Wgpu {
    pub async fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::default();
        // Request an adapter that can support our surface
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .expect("Failed to find an appropriate adapter");

        // Create logical device and command queue
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            })
            .await
            .expect("Failed to create device");
        println!("Prepared device: {:?}", device);

        Wgpu {
            adapter,
            device,
            queue,
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
