//! Main GPU renderer for the terminal grid.

use crate::font::{load_builtin_font, FontMetrics, LoadedFont};
use crate::glyph_cache::GlyphCache;
use termesh_terminal::grid::GridSnapshot;
use wgpu::util::DeviceExt;

/// Per-instance data sent to the GPU for each cell.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CellInstance {
    cell_pos: [f32; 2],
    cell_size: [f32; 2],
    fg_color: [f32; 4],
    bg_color: [f32; 4],
    uv_offset: [f32; 2],
    uv_size: [f32; 2],
    glyph_offset: [f32; 2],
    glyph_size: [f32; 2],
    has_glyph: f32,
    _padding: f32,
}

/// Uniform buffer data.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    projection: [[f32; 4]; 4],
}

/// GPU terminal renderer.
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    bg_pipeline: wgpu::RenderPipeline,
    glyph_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    atlas_texture: wgpu::Texture,
    atlas_bind_group: wgpu::BindGroup,
    font: LoadedFont,
    glyph_cache: GlyphCache,
    width: u32,
    height: u32,
}

impl Renderer {
    /// Create a new renderer for the given window surface.
    pub async fn new(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        font_size: f32,
    ) -> Result<Self, termesh_core::error::RenderError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window).map_err(|e| {
            termesh_core::error::RenderError::GpuInitFailed {
                reason: format!("surface creation failed: {e}"),
            }
        })?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or(termesh_core::error::RenderError::GpuInitFailed {
                reason: "no compatible GPU adapter found".to_string(),
            })?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("termesh"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    ..Default::default()
                },
                None,
            )
            .await
            .map_err(|e| termesh_core::error::RenderError::GpuInitFailed {
                reason: format!("device request failed: {e}"),
            })?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo, // VSync = 60fps
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Load font
        let font = load_builtin_font(font_size).map_err(|_| {
            termesh_core::error::RenderError::FontLoadFailed {
                path: "<builtin>".into(),
            }
        })?;

        // Create glyph cache + atlas texture
        let glyph_cache = GlyphCache::new();
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d {
                width: glyph_cache.atlas_width,
                height: glyph_cache.atlas_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terminal_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/terminal.wgsl").into()),
        });

        // Uniform buffer + bind group
        let projection = ortho_projection(width as f32, height as f32);
        let uniforms = Uniforms { projection };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniforms"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uniform_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Atlas texture bind group
        let atlas_view = atlas_texture.create_view(&Default::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("atlas_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas_bind_group"),
            layout: &atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        // Instance vertex layout
        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CellInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 32,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 48,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 56,
                    shader_location: 5,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 64,
                    shader_location: 6,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 72,
                    shader_location: 7,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 80,
                    shader_location: 8,
                },
            ],
        };

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &atlas_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Background pipeline (opaque)
        let bg_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bg_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_background"),
                buffers: std::slice::from_ref(&instance_layout),
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_background"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // Glyph pipeline (alpha blended)
        let glyph_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("glyph_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_glyph"),
                buffers: &[instance_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_glyph"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            bg_pipeline,
            glyph_pipeline,
            uniform_buffer,
            uniform_bind_group,
            atlas_texture,
            atlas_bind_group,
            font,
            glyph_cache,
            width,
            height,
        })
    }

    /// Resize the renderer surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);

        // Update projection matrix
        let projection = ortho_projection(width as f32, height as f32);
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[Uniforms { projection }]),
        );
    }

    /// Get font metrics.
    pub fn font_metrics(&self) -> FontMetrics {
        self.font.metrics
    }

    /// Render a terminal grid snapshot to the screen (full screen).
    pub fn render(&mut self, grid: &GridSnapshot) -> Result<(), wgpu::SurfaceError> {
        self.render_grids(&[(grid, 0.0, 0.0)])
    }

    /// Render multiple grid snapshots at different screen positions.
    ///
    /// Each entry is (grid, x_offset, y_offset) in pixel coordinates.
    pub fn render_grids(
        &mut self,
        grids: &[(&GridSnapshot, f32, f32)],
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&Default::default());

        let metrics = self.font.metrics;
        let atlas_w = self.glyph_cache.atlas_width as f32;
        let atlas_h = self.glyph_cache.atlas_height as f32;

        let mut bg_instances = Vec::new();
        let mut glyph_instances = Vec::new();

        for &(grid, x_offset, y_offset) in grids {
            for cell in &grid.cells {
                let x = x_offset + cell.col as f32 * metrics.cell_width;
                let y = y_offset + cell.row as f32 * metrics.cell_height;

                bg_instances.push(CellInstance {
                    cell_pos: [x, y],
                    cell_size: [metrics.cell_width, metrics.cell_height],
                    fg_color: cell.fg.to_f32_array(),
                    bg_color: cell.bg.to_f32_array(),
                    uv_offset: [0.0, 0.0],
                    uv_size: [0.0, 0.0],
                    glyph_offset: [0.0, 0.0],
                    glyph_size: [0.0, 0.0],
                    has_glyph: 0.0,
                    _padding: 0.0,
                });

                if cell.c != ' ' && cell.c != '\0' {
                    if let Some(glyph) = self.glyph_cache.get_or_insert(cell.c, &self.font) {
                        if glyph.width > 0 && glyph.height > 0 {
                            let glyph_x = glyph.bearing_x;
                            let glyph_y = metrics.baseline - glyph.bearing_y - glyph.height as f32;

                            glyph_instances.push(CellInstance {
                                cell_pos: [x, y],
                                cell_size: [metrics.cell_width, metrics.cell_height],
                                fg_color: cell.fg.to_f32_array(),
                                bg_color: cell.bg.to_f32_array(),
                                uv_offset: [
                                    glyph.atlas_x as f32 / atlas_w,
                                    glyph.atlas_y as f32 / atlas_h,
                                ],
                                uv_size: [
                                    glyph.width as f32 / atlas_w,
                                    glyph.height as f32 / atlas_h,
                                ],
                                glyph_offset: [glyph_x, glyph_y],
                                glyph_size: [glyph.width as f32, glyph.height as f32],
                                has_glyph: 1.0,
                                _padding: 0.0,
                            });
                        }
                    }
                }
            }
        }

        // Upload atlas if dirty
        if self.glyph_cache.dirty {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                self.glyph_cache.atlas_data(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.glyph_cache.atlas_width * 4),
                    rows_per_image: Some(self.glyph_cache.atlas_height),
                },
                wgpu::Extent3d {
                    width: self.glyph_cache.atlas_width,
                    height: self.glyph_cache.atlas_height,
                    depth_or_array_layers: 1,
                },
            );
            self.glyph_cache.mark_clean();
        }

        // Create GPU buffers
        let bg_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("bg_instances"),
                contents: bytemuck::cast_slice(&bg_instances),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let glyph_buffer = if !glyph_instances.is_empty() {
            Some(
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("glyph_instances"),
                        contents: bytemuck::cast_slice(&glyph_instances),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            )
        } else {
            None
        };

        // Encode render pass
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terminal_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.118,
                            g: 0.118,
                            b: 0.118,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            // Pass 1: backgrounds
            render_pass.set_pipeline(&self.bg_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &self.atlas_bind_group, &[]);
            render_pass.set_vertex_buffer(0, bg_buffer.slice(..));
            render_pass.draw(0..4, 0..bg_instances.len() as u32);

            // Pass 2: glyphs
            if let Some(ref buf) = glyph_buffer {
                render_pass.set_pipeline(&self.glyph_pipeline);
                render_pass.set_vertex_buffer(0, buf.slice(..));
                render_pass.draw(0..4, 0..glyph_instances.len() as u32);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    /// Get the surface dimensions.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Calculate terminal grid dimensions from the surface size.
    pub fn grid_size(&self) -> (usize, usize) {
        let metrics = self.font.metrics;
        let cols = (self.width as f32 / metrics.cell_width).floor() as usize;
        let rows = (self.height as f32 / metrics.cell_height).floor() as usize;
        (rows.max(1), cols.max(1))
    }
}

/// Build an orthographic projection matrix (top-left origin, pixel coords).
fn ortho_projection(width: f32, height: f32) -> [[f32; 4]; 4] {
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, -2.0 / height, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0, 1.0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ortho_projection() {
        let proj = ortho_projection(800.0, 600.0);
        // Top-left should map to (-1, 1) in clip space
        assert!((proj[0][0] - 2.0 / 800.0).abs() < 0.001);
        assert!((proj[1][1] - (-2.0 / 600.0)).abs() < 0.001);
        assert!((proj[3][0] - (-1.0)).abs() < 0.001);
        assert!((proj[3][1] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cell_instance_size() {
        // Ensure CellInstance is properly aligned for GPU
        assert_eq!(std::mem::size_of::<CellInstance>(), 88);
    }

    #[test]
    fn test_uniforms_size() {
        assert_eq!(std::mem::size_of::<Uniforms>(), 64);
    }
}
