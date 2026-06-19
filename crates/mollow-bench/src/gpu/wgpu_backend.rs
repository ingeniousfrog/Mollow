use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::BenchmarkError;

const SHADER: &str = r"
struct Params {
    size: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> matrix_a: array<f32>;
@group(0) @binding(2) var<storage, read> matrix_b: array<f32>;
@group(0) @binding(3) var<storage, read_write> matrix_c: array<f32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let row = gid.y;
    let col = gid.x;
    let n = params.size;
    if row >= n || col >= n {
        return;
    }
    var sum = 0.0;
    for (var k: u32 = 0u; k < n; k = k + 1u) {
        sum = sum + matrix_a[row * n + k] * matrix_b[k * n + col];
    }
    matrix_c[row * n + col] = sum;
}
";

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    size: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

pub struct WgpuMatrixBackend {
    pub adapter_name: String,
    pub api: String,
    device: wgpu::Device,
    queue: wgpu::Queue,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
    workgroups: (u32, u32),
}

impl WgpuMatrixBackend {
    pub fn initialize(size: usize) -> Result<Self, BenchmarkError> {
        let size_u32 =
            u32::try_from(size).map_err(|error| BenchmarkError::new("gpu", error.to_string()))?;
        pollster::block_on(Self::initialize_async(size_u32))
    }

    pub fn dispatch(&self) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mollow-gpu-matrix"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("mollow-gpu-matrix-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups(self.workgroups.0, self.workgroups.1, 1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.device.poll(wgpu::Maintain::wait());
    }

    #[allow(clippy::too_many_lines)]
    async fn initialize_async(size: u32) -> Result<Self, BenchmarkError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok_or_else(|| {
                BenchmarkError::new("gpu", "no compatible GPU adapter was found for wgpu")
            })?;
        let adapter_info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("mollow-gpu"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|error| BenchmarkError::new("gpu", error.to_string()))?;

        let matrix_a = deterministic_matrix(size, 1);
        let matrix_b = deterministic_matrix(size, 2);
        let matrix_c = vec![
            0.0_f32;
            usize::try_from(size)
                .map_err(|error| BenchmarkError::new("gpu", error.to_string()))?
                .pow(2)
        ];

        let buffer_a = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("matrix-a"),
            contents: bytemuck::cast_slice(&matrix_a),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let buffer_b = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("matrix-b"),
            contents: bytemuck::cast_slice(&matrix_b),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let buffer_c = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("matrix-c"),
            contents: bytemuck::cast_slice(&matrix_c),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("matrix-params"),
            contents: bytemuck::bytes_of(&Params {
                size,
                _pad0: 0,
                _pad1: 0,
                _pad2: 0,
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("matrix-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(std::mem::size_of::<Params>() as u64),
                    },
                    count: None,
                },
                storage_entry(1, true),
                storage_entry(2, true),
                storage_entry(3, false),
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("matrix-bind-group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffer_a.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer_b.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: buffer_c.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("matrix-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("matrix-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("matrix-pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let workgroup_size = 16_u32;
        let workgroups = (size.div_ceil(workgroup_size), size.div_ceil(workgroup_size));

        Ok(Self {
            adapter_name: adapter_info.name,
            api: format!("{:?}", adapter_info.backend),
            device,
            queue,
            bind_group,
            pipeline,
            workgroups,
        })
    }
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn deterministic_matrix(size: u32, seed: u32) -> Vec<f32> {
    let length = usize::try_from(size).unwrap_or(0).pow(2);
    let mut values = Vec::with_capacity(length);
    let mut state = seed;
    for _ in 0..length {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        values.push(f32::from((state & 0xff) as u8) / 255.0);
    }
    values
}
