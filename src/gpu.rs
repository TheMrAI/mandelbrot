use std::borrow::Cow;

use wgpu::{BindGroupEntry, BufferBinding, BufferUsages, Device, Queue};

pub struct Wgpu {
    pub device: Device,
    pub queue: Queue,
}

impl Wgpu {
    pub async fn new() -> Self {
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

        Wgpu { device, queue }
    }

    pub fn render(
        &mut self,
        buffer: &mut [u32],
        upper_left: (f32, f32),
        view_resolution: (f32, f32),
        window_resolution: &winit::dpi::PhysicalSize<u32>,
    ) {
        // PREPARE COMPUTE
        // allocate local texture representation
        let mut texture_data =
            vec![0u8; (window_resolution.width * window_resolution.height * 4) as usize];
        // Load the shaders
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("shader"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
            });

        // Storage texture for calculation output
        let storage_texture = self.device.create_texture(&wgpu::TextureDescriptor {
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
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        // TODO why default?
        let storage_texture_view =
            storage_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let output_staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output staging buffer"),
            size: size_of_val(&texture_data[..]) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Uniform buffer
        let uniform_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("settings_uniform"),
            size: 6 * size_of::<f32>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                });

        // Create bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: None, // use whole buffer
                    }),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&storage_texture_view),
                },
            ],
        });

        // Pipeline
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });
        let compute_pipeline =
            self.device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("mandelbrot compute pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                });

        self.queue.write_buffer(
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

        let mut command_encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("compute command encoder"),
                });
        {
            // run computation command
            let mut compute_pass =
                command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("compute pass"),
                    timestamp_writes: None,
                });
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.set_pipeline(&compute_pipeline);
            compute_pass.dispatch_workgroups(window_resolution.width, window_resolution.height, 1);
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
        self.queue.submit(Some(command_encoder.finish()));

        let buffer_slice = output_staging_buffer.slice(..);
        // TODO do you need to synchronize on the callback result or is it enough that your poll
        // has returned? for the time being I think it should be enough, but more investigation is
        // warranted
        buffer_slice.map_async(wgpu::MapMode::Read, move |_| {});
        self.device.poll(wgpu::PollType::Wait).unwrap();
        {
            let view = buffer_slice.get_mapped_range();
            texture_data.copy_from_slice(&view[..]);
        }
        output_staging_buffer.unmap();

        // this is rather nasty
        // softbuffer expects the value as ARGB while
        // the texture is produced as RGBA
        // TODO maybe we can do better?
        for row in 0..window_resolution.height {
            for column in 0..window_resolution.width {
                let texture_column_width = window_resolution.width * 4;
                let texture_index = ((row * texture_column_width) + column * 4) as usize;
                let shifted = (texture_data[texture_index] as u32) << 16
                    | (texture_data[texture_index + 1] as u32) << 8
                    | (texture_data[texture_index + 2] as u32);
                let pixel_index = (row * window_resolution.width + column) as usize;
                buffer[pixel_index] = shifted;
            }
        }
    }
}
