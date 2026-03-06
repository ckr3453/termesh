//! Main GPU renderer for the terminal grid.

use crate::font::{load_builtin_font, FontMetrics, LoadedFont};
use glyphon::{
    Buffer as GlyphonBuffer, Cache as GlyphonCache, Color as GlyphonColor, Metrics as GlyphonMetrics,
    Resolution, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use glyphon::cosmic_text::{Attrs, Family, Shaping};
use termesh_terminal::grid::GridSnapshot;
use unicode_width::UnicodeWidthStr;
use wgpu::util::DeviceExt;

/// Per-instance data sent to the GPU for each background/cursor/divider quad.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CellInstance {
    cell_pos: [f32; 2],
    cell_size: [f32; 2],
    bg_color: [f32; 4],
}

/// Uniform buffer data.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    projection: [[f32; 4]; 4],
}

/// Initial GPU buffer capacity in number of cell instances (80x24 grid).
const INITIAL_BUFFER_CAPACITY: usize = 1920;

/// GPU terminal renderer.
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    bg_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    font: LoadedFont,
    width: u32,
    height: u32,
    /// Frame counter for cursor blink (toggles every ~30 frames at 60fps = 0.5s).
    frame_count: u32,
    /// Reusable per-frame buffers to avoid allocation every frame.
    bg_buf: Vec<CellInstance>,
    cursor_buf: Vec<CellInstance>,
    /// Pre-allocated GPU vertex buffer for background instances.
    bg_gpu_buffer: wgpu::Buffer,
    bg_gpu_capacity: usize,
    /// Pre-allocated GPU vertex buffer for cursor instances.
    cursor_gpu_buffer: wgpu::Buffer,
    cursor_gpu_capacity: usize,
    /// glyphon text rendering.
    /// Kept alive for TextAtlas and Viewport bind group ownership.
    _glyphon_cache: GlyphonCache,
    text_atlas: TextAtlas,
    text_renderer: TextRenderer,
    viewport: Viewport,
    /// Reusable row buffers for glyphon text layout.
    row_buffers: Vec<GlyphonBuffer>,
    /// Reusable per-frame span storage to avoid allocation.
    span_buf: Vec<(String, GlyphonColor)>,
    /// Per-row metadata for TextArea construction.
    row_metas: Vec<RowMeta>,
    /// Previous total row count for detecting layout changes.
    prev_total_rows: usize,
    /// Previous selection range for detecting selection changes.
    prev_selection: Option<termesh_terminal::grid::SelectionRange>,
}

/// Metadata for positioning a row's TextArea.
struct RowMeta {
    x_offset: f32,
    y_offset: f32,
    grid_cols: usize,
}

/// IME preedit overlay information for rendering.
pub struct PreeditOverlay {
    /// The preedit text to display.
    pub text: String,
    /// Pixel X position (left edge).
    pub x: f32,
    /// Pixel Y position (top edge).
    pub y: f32,
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

        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
        {
            Ok(adapter) => adapter,
            Err(_) => {
                log::warn!("no hardware GPU adapter found, trying software fallback");
                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface: Some(&surface),
                        force_fallback_adapter: true,
                    })
                    .await
                    .map_err(|_| termesh_core::error::RenderError::GpuInitFailed {
                        reason: "no compatible GPU adapter found (hardware or software)"
                            .to_string(),
                    })?
            }
        };

        let info = adapter.get_info();
        log::info!(
            "GPU adapter: {} ({:?}, {:?})",
            info.name,
            info.backend,
            info.device_type
        );

        let (device, queue): (wgpu::Device, wgpu::Queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("termesh"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|e| termesh_core::error::RenderError::GpuInitFailed {
                reason: format!("device request failed: {e}"),
            })?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
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

        // Instance vertex layout (simplified: pos + size + bg_color)
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
            ],
        };

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bg_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            immediate_size: 0,
        });

        // Background pipeline (opaque, used for bg/cursor/dividers)
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
            multiview_mask: None,
            cache: None,
        });

        let instance_size = std::mem::size_of::<CellInstance>();
        let bg_gpu_buffer =
            create_vertex_buffer(&device, "bg_instances", INITIAL_BUFFER_CAPACITY * instance_size);
        let cursor_gpu_buffer =
            create_vertex_buffer(&device, "cursor_instances", 4 * instance_size);

        // glyphon setup
        let glyphon_cache = GlyphonCache::new(&device);
        let mut text_atlas = TextAtlas::new(&device, &queue, &glyphon_cache, surface_format);
        let text_renderer =
            TextRenderer::new(&mut text_atlas, &device, Default::default(), None);
        let mut viewport = Viewport::new(&device, &glyphon_cache);
        viewport.update(&queue, Resolution { width, height });

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            bg_pipeline,
            uniform_buffer,
            uniform_bind_group,
            font,
            width,
            height,
            frame_count: 0,
            bg_buf: Vec::new(),
            cursor_buf: Vec::new(),
            bg_gpu_buffer,
            bg_gpu_capacity: INITIAL_BUFFER_CAPACITY,
            cursor_gpu_buffer,
            cursor_gpu_capacity: 4,
            _glyphon_cache: glyphon_cache,
            text_atlas,
            text_renderer,
            viewport,
            row_buffers: Vec::new(),
            span_buf: Vec::new(),
            row_metas: Vec::new(),
            prev_total_rows: 0,
            prev_selection: None,
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

        // Update glyphon viewport
        self.viewport
            .update(&self.queue, Resolution { width, height });
    }

    /// Get font metrics.
    pub fn font_metrics(&self) -> FontMetrics {
        self.font.metrics
    }

    /// Ensure the background GPU buffer can hold `needed` instances.
    fn ensure_bg_capacity(&mut self, needed: usize) {
        if needed > self.bg_gpu_capacity {
            let new_cap = (self.bg_gpu_capacity * 2).max(needed);
            self.bg_gpu_buffer = create_vertex_buffer(
                &self.device,
                "bg_instances",
                new_cap * std::mem::size_of::<CellInstance>(),
            );
            self.bg_gpu_capacity = new_cap;
            log::debug!("bg_gpu_buffer grown to {new_cap} instances");
        }
    }

    /// Ensure the cursor GPU buffer can hold `needed` instances.
    fn ensure_cursor_capacity(&mut self, needed: usize) {
        if needed > self.cursor_gpu_capacity {
            let new_cap = (self.cursor_gpu_capacity * 2).max(needed);
            self.cursor_gpu_buffer = create_vertex_buffer(
                &self.device,
                "cursor_instances",
                new_cap * std::mem::size_of::<CellInstance>(),
            );
            self.cursor_gpu_capacity = new_cap;
            log::debug!("cursor_gpu_buffer grown to {new_cap} instances");
        }
    }

    /// Render a terminal grid snapshot to the screen (full screen).
    pub fn render(&mut self, grid: &GridSnapshot) -> Result<(), wgpu::SurfaceError> {
        self.render_grids(&[(grid, 0.0, 0.0)], &[], None)
    }

    /// Render multiple grid snapshots at different screen positions.
    pub fn render_grids(
        &mut self,
        grids: &[(&GridSnapshot, f32, f32)],
        dividers: &[(f32, f32, f32, bool, [f32; 4])],
        preedit: Option<&PreeditOverlay>,
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&Default::default());

        let metrics = self.font.metrics;

        // Cursor blink
        self.frame_count = self.frame_count.wrapping_add(1);
        let cursor_visible = (self.frame_count / 30).is_multiple_of(2);

        self.bg_buf.clear();
        self.cursor_buf.clear();
        self.row_metas.clear();

        // Count total rows across all grids for row buffer allocation
        // +1 for potential preedit overlay row
        let total_rows: usize = grids.iter().map(|(g, _, _)| g.rows).sum();
        let need_rows = total_rows + if preedit.is_some() { 1 } else { 0 };

        // Ensure we have enough row buffers
        {
            let mut fs = self.font.font_system_mut();
            let glyphon_metrics =
                GlyphonMetrics::new(metrics.font_size, metrics.cell_height);
            while self.row_buffers.len() < need_rows {
                self.row_buffers
                    .push(GlyphonBuffer::new(&mut fs, glyphon_metrics));
            }
        }

        // Detect layout or selection changes that invalidate cached row buffers.
        let layout_changed = total_rows != self.prev_total_rows;
        let selection_changed = {
            let current_sel = grids.first().and_then(|(g, _, _)| g.selection);
            let changed = current_sel != self.prev_selection;
            self.prev_selection = current_sel;
            changed
        };
        self.prev_total_rows = total_rows;

        // Phase 1: Build backgrounds, cursor, and populate row buffers
        let mut row_buf_idx = 0;

        for &(grid, x_offset, y_offset) in grids {
            // Cursor block
            if cursor_visible && grid.cursor.visible {
                let cx = x_offset + grid.cursor.col as f32 * metrics.cell_width;
                let cy = y_offset + grid.cursor.row as f32 * metrics.cell_height;
                self.cursor_buf.push(CellInstance {
                    cell_pos: [cx, cy],
                    cell_size: [metrics.cell_width, metrics.cell_height],
                    bg_color: [0.8, 0.8, 0.8, 1.0],
                });
            }

            for row in 0..grid.rows {
                // Determine if this row needs text re-shaping.
                let row_dirty = layout_changed
                    || selection_changed
                    || grid
                        .dirty_rows
                        .as_ref()
                        .map_or(true, |dr| dr.get(row).copied().unwrap_or(true));

                self.span_buf.clear();
                let mut last_col: usize = 0;

                for col in 0..grid.cols {
                    let cell = &grid.cells[row * grid.cols + col];

                    if cell.spacer {
                        continue;
                    }

                    let x = (x_offset + cell.col as f32 * metrics.cell_width).floor();
                    let y = (y_offset + cell.row as f32 * metrics.cell_height).floor();
                    let stride = cell.width as usize;
                    let next_x =
                        (x_offset + (cell.col + stride) as f32 * metrics.cell_width).floor();
                    let next_y =
                        (y_offset + (cell.row + 1) as f32 * metrics.cell_height).floor();
                    let bg_w = next_x - x;
                    let bg_h = next_y - y;

                    let selected = grid.selection.is_some_and(|sel| {
                        let r = cell.row;
                        let c = cell.col;
                        if r < sel.start_row || r > sel.end_row {
                            false
                        } else if r == sel.start_row && r == sel.end_row {
                            c >= sel.start_col && c <= sel.end_col
                        } else if r == sel.start_row {
                            c >= sel.start_col
                        } else if r == sel.end_row {
                            c <= sel.end_col
                        } else {
                            true
                        }
                    });

                    let (fg_rgba, bg) = if selected {
                        (cell.bg, [0.2, 0.4, 0.8, 1.0])
                    } else {
                        (cell.fg, cell.bg.to_f32_array())
                    };

                    self.bg_buf.push(CellInstance {
                        cell_pos: [x, y],
                        cell_size: [bg_w, bg_h],
                        bg_color: bg,
                    });

                    if row_dirty {
                        // Fill gaps with transparent spaces for skipped spacer columns
                        while last_col < col {
                            self.span_buf
                                .push((" ".to_string(), GlyphonColor::rgba(0, 0, 0, 0)));
                            last_col += 1;
                        }

                        let color =
                            GlyphonColor::rgba(fg_rgba.r, fg_rgba.g, fg_rgba.b, fg_rgba.a);
                        if cell.c == '\0' {
                            self.span_buf.push((" ".to_string(), color));
                        } else {
                            self.span_buf.push((cell.c.to_string(), color));
                        }
                        last_col = col + stride;
                    }
                }

                // Only re-shape text for dirty rows; clean rows reuse previous buffer
                if row_dirty {
                    let buf = &mut self.row_buffers[row_buf_idx];
                    let mut fs = self.font.font_system_mut();
                    let glyphon_metrics =
                        GlyphonMetrics::new(metrics.font_size, metrics.cell_height);
                    buf.set_metrics(&mut fs, glyphon_metrics);
                    buf.set_size(
                        &mut fs,
                        Some(metrics.cell_width * grid.cols as f32),
                        Some(metrics.cell_height),
                    );

                    let default_attrs = Attrs::new().family(Family::Monospace);
                    let spans: Vec<(&str, Attrs)> = self
                        .span_buf
                        .iter()
                        .map(|(s, color)| {
                            (
                                s.as_str(),
                                Attrs::new().family(Family::Monospace).color(*color),
                            )
                        })
                        .collect();
                    buf.set_rich_text(
                        &mut fs,
                        spans,
                        &default_attrs,
                        Shaping::Advanced,
                        None,
                    );
                    buf.shape_until_scroll(&mut fs, false);
                }

                self.row_metas.push(RowMeta {
                    x_offset,
                    y_offset: y_offset + row as f32 * metrics.cell_height,
                    grid_cols: grid.cols,
                });
                row_buf_idx += 1;
            }
        }

        // Preedit overlay: populate extra row buffer + bg/underline quads
        // Preedit bg/underline are appended to cursor_buf after the cursor block,
        // so they overdraw the cursor at the composition position (intended).
        if let Some(pe) = preedit {
            let display_width = pe.text.width();
            if display_width == 0 {
                // Zero-width preedit (e.g., combining-only chars) — skip overlay
            } else {
            let pe_w = display_width as f32 * metrics.cell_width;
            let underline_h = 2.0_f32;

            // Semi-transparent background
            self.cursor_buf.push(CellInstance {
                cell_pos: [pe.x, pe.y],
                cell_size: [pe_w, metrics.cell_height],
                bg_color: [0.15, 0.15, 0.25, 0.92],
            });
            // Underline indicator
            self.cursor_buf.push(CellInstance {
                cell_pos: [pe.x, pe.y + metrics.cell_height - underline_h],
                cell_size: [pe_w, underline_h],
                bg_color: [0.4, 0.6, 1.0, 1.0],
            });

            // Populate preedit text in extra row buffer
            let buf = &mut self.row_buffers[row_buf_idx];
            {
                let mut fs = self.font.font_system_mut();
                let glyphon_metrics =
                    GlyphonMetrics::new(metrics.font_size, metrics.cell_height);
                buf.set_metrics(&mut fs, glyphon_metrics);
                buf.set_size(&mut fs, Some(pe_w), Some(metrics.cell_height));

                let default_attrs = Attrs::new().family(Family::Monospace);
                let spans = [(
                    pe.text.as_str(),
                    Attrs::new()
                        .family(Family::Monospace)
                        .color(GlyphonColor::rgba(255, 255, 255, 255)),
                )];
                buf.set_rich_text(
                    &mut fs,
                    spans,
                    &default_attrs,
                    Shaping::Advanced,
                    None,
                );
                buf.shape_until_scroll(&mut fs, false);
            }

            self.row_metas.push(RowMeta {
                x_offset: pe.x,
                y_offset: pe.y,
                grid_cols: display_width,
            });
            row_buf_idx += 1;
            } // else display_width > 0
        }

        // Phase 2: Create TextAreas from (now immutable) row buffers and prepare glyphon
        {
            let text_areas: Vec<TextArea> = self
                .row_buffers
                .iter()
                .zip(self.row_metas.iter())
                .take(row_buf_idx)
                .map(|(buf, meta)| TextArea {
                    buffer: buf,
                    left: meta.x_offset,
                    top: meta.y_offset,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: meta.x_offset as i32,
                        top: meta.y_offset as i32,
                        right: (meta.x_offset + meta.grid_cols as f32 * metrics.cell_width)
                            as i32,
                        bottom: (meta.y_offset + metrics.cell_height) as i32 + 1,
                    },
                    default_color: GlyphonColor::rgba(204, 204, 204, 255),
                    custom_glyphs: &[],
                })
                .collect();

            let mut fs = self.font.font_system_mut();
            let mut cache = self.font.swash_cache_mut();
            self.text_renderer
                .prepare(
                    &self.device,
                    &self.queue,
                    &mut fs,
                    &mut self.text_atlas,
                    &self.viewport,
                    text_areas,
                    &mut cache,
                )
                .unwrap_or_else(|e| log::warn!("glyphon prepare failed: {e:?}"));
        }

        // Write instance data to pre-allocated GPU buffers
        if !self.bg_buf.is_empty() {
            self.ensure_bg_capacity(self.bg_buf.len());
            self.queue.write_buffer(
                &self.bg_gpu_buffer,
                0,
                bytemuck::cast_slice(&self.bg_buf),
            );
        }

        if !self.cursor_buf.is_empty() {
            self.ensure_cursor_capacity(self.cursor_buf.len());
            self.queue.write_buffer(
                &self.cursor_gpu_buffer,
                0,
                bytemuck::cast_slice(&self.cursor_buf),
            );
        }

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
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            // Pass 1: backgrounds
            render_pass.set_pipeline(&self.bg_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.bg_gpu_buffer.slice(..));
            render_pass.draw(0..4, 0..self.bg_buf.len() as u32);

            // Pass 2: text (glyphon)
            self.text_renderer
                .render(&self.text_atlas, &self.viewport, &mut render_pass)
                .unwrap_or_else(|e| log::warn!("glyphon render failed: {e:?}"));

            // Pass 3: cursor overlay
            if !self.cursor_buf.is_empty() {
                render_pass.set_pipeline(&self.bg_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.cursor_gpu_buffer.slice(..));
                render_pass.draw(0..4, 0..self.cursor_buf.len() as u32);
            }

            // Pass 4: pane dividers
            if !dividers.is_empty() {
                let thickness = 1.0_f32;
                let divider_instances: Vec<CellInstance> = dividers
                    .iter()
                    .map(|&(x, y, length, is_vertical, color)| {
                        let (w, h) = if is_vertical {
                            (thickness, length)
                        } else {
                            (length, thickness)
                        };
                        CellInstance {
                            cell_pos: [x, y],
                            cell_size: [w, h],
                            bg_color: color,
                        }
                    })
                    .collect();
                let divider_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("divider_instances"),
                            contents: bytemuck::cast_slice(&divider_instances),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                render_pass.set_pipeline(&self.bg_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, divider_buffer.slice(..));
                render_pass.draw(0..4, 0..divider_instances.len() as u32);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        self.text_atlas.trim();

        Ok(())
    }

    /// Get the surface dimensions.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get the surface texture format being used.
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_config.format
    }

    /// Calculate terminal grid dimensions from the surface size.
    pub fn grid_size(&self) -> (usize, usize) {
        let metrics = self.font.metrics;
        let cols = (self.width as f32 / metrics.cell_width).floor() as usize;
        let rows = (self.height as f32 / metrics.cell_height).floor() as usize;
        (rows.max(1), cols.max(1))
    }
}

/// Create a pre-allocated GPU vertex buffer with the given byte size.
fn create_vertex_buffer(device: &wgpu::Device, label: &str, size: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
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
        assert!((proj[0][0] - 2.0 / 800.0).abs() < 0.001);
        assert!((proj[1][1] - (-2.0 / 600.0)).abs() < 0.001);
        assert!((proj[3][0] - (-1.0)).abs() < 0.001);
        assert!((proj[3][1] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cell_instance_size() {
        // Simplified: pos(8) + size(8) + bg_color(16) = 32 bytes
        assert_eq!(std::mem::size_of::<CellInstance>(), 32);
    }

    #[test]
    fn test_uniforms_size() {
        assert_eq!(std::mem::size_of::<Uniforms>(), 64);
    }
}
