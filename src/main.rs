use std::{borrow::Cow, sync::Arc};

use wgpu::{Device, Queue, RenderPipeline, Surface};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, ElementState, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

mod cpu;

#[derive(Default)]
struct App {
    app: Option<InnerApp>,
}

struct InnerApp {
    pub window: Arc<Window>,
    pub gpu: Wgpu,

    pub focused: bool,
    pub left_mouse: ElementState,
}

impl InnerApp {
    pub fn new(event_loop: &winit::event_loop::ActiveEventLoop) -> Self {
        let window_attributes = Window::default_attributes()
            .with_title("Mandelbrot")
            .with_resizable(false)
            .with_inner_size(winit::dpi::LogicalSize::new(1024.0, 768.0));

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        let gpu = pollster::block_on(Wgpu::new(Arc::clone(&window)));

        InnerApp {
            window,
            gpu,
            focused: true,
            left_mouse: ElementState::Released,
        }
    }
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
                if let Some(app) = self.app.as_ref() {
                    let frame = app
                        .gpu
                        .surface
                        .get_current_texture()
                        .expect("Failed to acquire next swap-chain texture.");

                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder =
                        app.gpu
                            .device
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("encoder"),
                            });

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
                if let Some(app) = self.app.as_ref() {
                    if app.focused {
                        println!("{:?} MouseWheel delta: {:?}", device_id, delta);
                    }
                }
            }
            DeviceEvent::MouseMotion { delta } => {
                if let Some(app) = self.app.as_ref() {
                    if app.focused && app.left_mouse == ElementState::Pressed {
                        println!("{:?} MouseMotion delta: {:?}", device_id, delta);
                    }
                }
            }
            DeviceEvent::Button { button, state } => {
                if let Some(app) = self.app.as_mut() {
                    if button == 0 {
                        app.left_mouse = state;
                    }
                    println!("{:?} {:?}", button, state);
                }
            }
            _ => {}
        }
    }
}

struct Wgpu {
    pub surface: Surface<'static>,
    pub device: Device,
    pub render_pipeline: RenderPipeline,
    pub queue: Queue,
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

        // Pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[],
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
            surface,
            device,
            render_pipeline,
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
