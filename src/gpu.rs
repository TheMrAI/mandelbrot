use std::borrow::Cow;

use num::Complex;
use wgpu::{BindGroupEntry, BufferBinding, BufferUsages, Device, Queue};
use winit::dpi::PhysicalSize;

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
        upper_left: Complex<f32>,
        view_resolution: &PhysicalSize<f32>,
        window_resolution: &PhysicalSize<u32>,
    ) {
        // PREPARE COMPUTE

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
            size: size_of_val(buffer) as u64,
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
                upper_left.re,
                upper_left.im,
                view_resolution.width,
                view_resolution.height,
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
            // The incoming texel data has byte order RGBA, and the softbuffer expects it to be in
            // 0RGB (no alpha, first byte completely zero)
            // Ideally it would be best if we could just take the mapped buffer_slice and
            // [transmute_copy](https://doc.rust-lang.org/std/mem/fn.transmute_copy.html) it into the buffer, but this
            // wouldn't help as we would have to go through the bytes anyways and shift them 8 bits to the right, to be
            // in the correct format. We could also just cast the buffer_slice as an *u32 ptr step through the elements
            // and copy the shifted values into the softbuffer buffer.
            // Neither of these options will work, because the moment an u8 slice is reinterpreted as a u32 slice
            // (same for raw pointers) the stored byte order will change.
            // 0xFF00FF00 will become 0x00FF00FF, the issue comes from the endianess of the u32 on your system.
            // With u32::from_be_bytes, u32::from_le_bytes you can reliably recast a 4 bytes into an u32, but you must
            // know the appropriate endiannes. This same issue comes when calling transmute functions, the byte order
            // will change. So we simply construct the u32 values by hand and sidestep this problem altogether. While
            // it doesn't appear very efficient it seems to get pretty well optimized, and in practice couldn't observe
            // much overhead (if any), when compared to simply casting/copying memory.
            // Why does the order of bytes change when casting the u8 ptr to u32 mess with memory order of the bytes is
            // a mystery.
            for (buffer_index, item) in buffer.iter_mut().enumerate() {
                let view_index = buffer_index * 4;
                *item = (view[view_index] as u32) << 16
                    | (view[view_index + 1] as u32) << 8
                    | (view[view_index + 2] as u32);
            }
        }
        output_staging_buffer.unmap();
    }
}
