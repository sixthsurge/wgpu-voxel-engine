use wgpu::BufferAddress;

use crate::{
    camera::Camera,
    render::{Mesh, Vertex, WgpuState},
};

pub struct WorldRenderer {
    chunk_meshes: Vec<Mesh>,
    terrain_pipeline: wgpu::RenderPipeline,
    global_uniforms: GlobalUniforms,
    global_uniforms_buffer: wgpu::Buffer,
    global_uniforms_bind_group: wgpu::BindGroup,
}

impl WorldRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let chunk_meshes = Vec::new();

        let global_uniforms = GlobalUniforms::default();

        let global_uniforms_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Global Uniform Buffer"),
            size: std::mem::size_of::<GlobalUniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let global_uniforms_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let global_uniforms_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &global_uniforms_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: global_uniforms_buffer.as_entire_binding(),
            }],
            label: Some("Global Uniforms Bind Group"),
        });

        let terrain_shader =
            device.create_shader_module(wgpu::include_wgsl!("shaders/terrain.wgsl"));

        let terrain_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&global_uniforms_bind_group_layout],
                push_constant_ranges: &[],
            });

        let terrain_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Terrain Pipeline"),
            layout: Some(&terrain_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &terrain_shader,
                entry_point: "vs_main",
                buffers: &[TerrainVertex::vertex_buffer_layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &terrain_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    write_mask: wgpu::ColorWrites::ALL,
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Self {
            chunk_meshes,
            terrain_pipeline,
            global_uniforms,
            global_uniforms_buffer,
            global_uniforms_bind_group,
        }
    }

    pub fn render(&mut self, wgpu: &WgpuState, surface_texture_view: &wgpu::TextureView) {
        wgpu.queue.write_buffer(
            &self.global_uniforms_buffer,
            0 as BufferAddress,
            bytemuck::cast_slice(&[self.global_uniforms]),
        );

        let mut encoder = wgpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("World Render Encoder"),
            });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Terrain Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &surface_texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(&self.terrain_pipeline);
        render_pass.set_bind_group(0, &self.global_uniforms_bind_group, &[]);

        for mesh in &self.chunk_meshes {
            mesh.draw(&mut render_pass);
        }

        drop(render_pass);

        let command_buffer = encoder.finish();

        wgpu.queue.submit(std::iter::once(command_buffer));
    }

    pub fn add_chunk_mesh(&mut self, mesh: Mesh) {
        self.chunk_meshes.push(mesh);
    }

    pub fn set_camera(&mut self, camera: &Camera) {
        self.global_uniforms.camera_view_matrix = camera.view_matrix().to_cols_array_2d();
        self.global_uniforms.camera_projection_matrix =
            camera.projection_matrix().to_cols_array_2d();
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TerrainVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

impl Vertex for TerrainVertex {
    fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobalUniforms {
    pub camera_view_matrix: [[f32; 4]; 4],
    pub camera_projection_matrix: [[f32; 4]; 4],
}
