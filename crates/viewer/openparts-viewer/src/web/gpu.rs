//! The wgpu renderer: the shaded model, then line layers (`Layer`): the
//! board-plane grid and the dimensions depth tested, a faint always-on-top
//! ghost of the dimensions, and the measurement always on
//! top). Lines are screen-space quads so they keep a pixel width.

use crate::mesh::Mesh;
use glam::{Mat4, Vec2, Vec3};
use wgpu::util::DeviceExt;

const SAMPLES: u32 = 4;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

const SHADER: &str = r#"
struct U {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    // physical width, height; 1 when the target isn't sRGB (encode here)
    viewport: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: U;

fn encode(c: vec3<f32>) -> vec3<f32> {
    if (u.viewport.z > 0.5) {
        return pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
    }
    return c;
}

struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

@vertex
fn vs_mesh(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>,
           @location(2) color: vec3<f32>) -> MeshOut {
    var o: MeshOut;
    o.clip = u.view_proj * vec4<f32>(pos, 1.0);
    o.world = pos;
    o.normal = normal;
    o.color = color;
    return o;
}

@fragment
fn fs_mesh(f: MeshOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var n = normalize(f.normal);
    if (!front) { n = -n; }
    let v = normalize(u.eye.xyz - f.world);
    let key = normalize(vec3<f32>(0.45, -0.55, 0.85));
    let sky = mix(vec3<f32>(0.32, 0.31, 0.30), vec3<f32>(0.80, 0.84, 0.92), n.z * 0.5 + 0.5);
    let diffuse = max(dot(n, key), 0.0);
    let head = max(dot(n, v), 0.0);
    let h = normalize(key + v);
    let spec = pow(max(dot(n, h), 0.0), 48.0) * 0.25;
    let c = f.color * (sky * 0.55 + vec3<f32>(diffuse * 0.55 + head * 0.30)) + vec3<f32>(spec);
    return vec4<f32>(encode(c), 1.0);
}

struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_line(@location(0) a: vec3<f32>, @location(1) b: vec3<f32>,
           @location(2) params: vec3<f32>, @location(3) color: vec4<f32>) -> LineOut {
    // params: side (-1/1), end (0 = a, 1 = b), width (physical px)
    let ca = u.view_proj * vec4<f32>(a, 1.0);
    let cb = u.view_proj * vec4<f32>(b, 1.0);
    let half_vp = u.viewport.xy * 0.5;
    let sa = ca.xy / max(ca.w, 1e-6) * half_vp;
    let sb = cb.xy / max(cb.w, 1e-6) * half_vp;
    var dir = sb - sa;
    if (length(dir) < 1e-6) { dir = vec2<f32>(1.0, 0.0); }
    dir = normalize(dir);
    let nrm = vec2<f32>(-dir.y, dir.x);
    var p = ca;
    if (params.y > 0.5) { p = cb; }
    let w = params.z * 0.5;
    let off = nrm * params.x * w + dir * (params.y * 2.0 - 1.0) * w;
    var o: LineOut;
    // A nudge toward the viewer, so lines on a face aren't lost in it.
    o.clip = vec4<f32>(p.xy + off / half_vp * p.w, p.z - 0.0004 * p.w, p.w);
    o.color = color;
    return o;
}

@fragment
fn fs_line(f: LineOut) -> @location(0) vec4<f32> {
    return vec4<f32>(encode(f.color.rgb) * f.color.a, f.color.a);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    viewport: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshVertex {
    pos: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LineVertex {
    a: [f32; 3],
    b: [f32; 3],
    params: [f32; 3],
    color: [f32; 4],
}

/// The line layers, drawn in this order after the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Depth tested: the board-plane grid.
    Grid = 0,
    /// Always on top, faint: dimensions where the model hides them.
    Ghost = 1,
    /// Depth tested: the dimensions.
    Dims = 2,
    /// Always on top: the measurement and its guides.
    Overlay = 3,
}

/// A line segment to draw: ends, linear RGBA colour, width in CSS px.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub a: Vec3,
    pub b: Vec3,
    pub color: [f32; 4],
    pub width: f32,
}

fn line_vertices(lines: &[Line], dpr: f32) -> Vec<LineVertex> {
    let mut v = Vec::with_capacity(lines.len() * 6);
    for l in lines {
        for (side, end) in [
            (-1., 0.),
            (1., 0.),
            (1., 1.),
            (-1., 0.),
            (1., 1.),
            (-1., 1.),
        ] {
            v.push(LineVertex {
                a: l.a.into(),
                b: l.b.into(),
                params: [side, end, l.width * dpr],
                color: l.color,
            });
        }
    }
    v
}

pub struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    encode_srgb: bool,
    pub backend: &'static str,
    mesh_pipeline: wgpu::RenderPipeline,
    grid_pipeline: wgpu::RenderPipeline,
    overlay_pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    mesh: Option<(wgpu::Buffer, u32)>,
    layers: [Option<(wgpu::Buffer, u32)>; 4],
    targets: Option<(wgpu::TextureView, wgpu::TextureView)>,
}

/// WebGPU when the browser has it, else WebGL2. `make_canvas` gives a
/// fresh canvas: one that already has a WebGPU context can't get WebGL.
pub async fn init(
    make_canvas: &dyn Fn() -> web_sys::HtmlCanvasElement,
) -> Result<(Gpu, web_sys::HtmlCanvasElement), String> {
    let mut last = String::from("no graphics adapter");
    for backends in [
        wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
        wgpu::Backends::GL,
    ] {
        let canvas = make_canvas();
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = backends;
        let instance = wgpu::util::new_instance_with_webgpu_detection(desc).await;
        let surface = match instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone())) {
            Ok(s) => s,
            Err(e) => {
                last = e.to_string();
                continue;
            }
        };
        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                apply_limit_buckets: false,
            })
            .await
        {
            Ok(a) => a,
            Err(e) => {
                last = e.to_string();
                continue;
            }
        };
        let backend = match adapter.get_info().backend {
            wgpu::Backend::BrowserWebGpu => "WebGPU",
            wgpu::Backend::Gl => "WebGL2",
            _ => "?",
        };
        let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        let (device, queue) = match adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("openparts-viewer"),
                required_limits: limits,
                ..Default::default()
            })
            .await
        {
            Ok(d) => d,
            Err(e) => {
                last = e.to_string();
                continue;
            }
        };
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let Some(mut config) = surface.get_default_config(&adapter, width, height) else {
            last = "the canvas can't be rendered to".into();
            continue;
        };
        let caps = surface.get_capabilities(&adapter);
        if let Some(f) = caps.formats.iter().find(|f| f.is_srgb()) {
            config.format = *f;
        }
        config.present_mode = wgpu::PresentMode::Fifo;
        surface.configure(&device, &config);
        let gpu = Gpu::new(surface, device, queue, config, backend);
        return Ok((gpu, canvas));
    }
    Err(last)
}

impl Gpu {
    fn new(
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        backend: &'static str,
    ) -> Gpu {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viewer"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniforms"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viewer"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let mesh_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];
        let line_attrs = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x4
        ];
        let pipeline = |entry: (&str, &str),
                        attrs: &[wgpu::VertexAttribute],
                        stride: usize,
                        depth_write: bool,
                        compare: wgpu::CompareFunction,
                        blend: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry.0),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(entry.0),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: stride as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: attrs,
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(depth_write),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry.1),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: blend.then_some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let mesh_pipeline = pipeline(
            ("vs_mesh", "fs_mesh"),
            &mesh_attrs,
            std::mem::size_of::<MeshVertex>(),
            true,
            wgpu::CompareFunction::Less,
            false,
        );
        let grid_pipeline = pipeline(
            ("vs_line", "fs_line"),
            &line_attrs,
            std::mem::size_of::<LineVertex>(),
            false,
            wgpu::CompareFunction::LessEqual,
            true,
        );
        let overlay_pipeline = pipeline(
            ("vs_line", "fs_line"),
            &line_attrs,
            std::mem::size_of::<LineVertex>(),
            false,
            wgpu::CompareFunction::Always,
            true,
        );
        Gpu {
            encode_srgb: !config.format.is_srgb(),
            surface,
            device,
            queue,
            config,
            backend,
            mesh_pipeline,
            grid_pipeline,
            overlay_pipeline,
            uniforms,
            bind_group,
            mesh: None,
            layers: Default::default(),
            targets: None,
        }
    }

    pub fn set_mesh(&mut self, mesh: &Mesh) {
        let v: Vec<MeshVertex> = (0..mesh.positions.len())
            .map(|i| MeshVertex {
                pos: mesh.positions[i].into(),
                normal: mesh.normals[i].into(),
                color: mesh.colors[i],
            })
            .collect();
        self.mesh = self.buffer(bytemuck::cast_slice(&v), v.len());
    }

    pub fn set_lines(&mut self, layer: Layer, lines: &[Line], dpr: f32) {
        let v = line_vertices(lines, dpr);
        self.layers[layer as usize] = self.buffer(bytemuck::cast_slice(&v), v.len());
    }

    fn buffer(&self, bytes: &[u8], count: usize) -> Option<(wgpu::Buffer, u32)> {
        (count > 0).then(|| {
            let b = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytes,
                    usage: wgpu::BufferUsages::VERTEX,
                });
            (b, count as u32)
        })
    }

    /// Follows the canvas's size (physical pixels).
    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if width != self.config.width || height != self.config.height {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.targets = None;
        }
    }

    fn targets(&mut self) -> &(wgpu::TextureView, wgpu::TextureView) {
        let (w, h, format) = (self.config.width, self.config.height, self.config.format);
        let device = &self.device;
        self.targets.get_or_insert_with(|| {
            let texture = |format, label| {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: SAMPLES,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            (texture(format, "msaa"), texture(DEPTH, "depth"))
        })
    }

    pub fn render(&mut self, view_proj: Mat4, eye: Vec3, background: [f64; 3]) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            _ => return,
        };
        let u = Uniforms {
            view_proj: view_proj.to_cols_array_2d(),
            eye: eye.extend(1.0).into(),
            viewport: [
                self.config.width as f32,
                self.config.height as f32,
                if self.encode_srgb { 1.0 } else { 0.0 },
                0.0,
            ],
        };
        self.queue
            .write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
        let target = frame.texture.create_view(&Default::default());
        let (msaa, depth) = {
            let t = self.targets();
            (t.0.clone(), t.1.clone())
        };
        let bg = if self.encode_srgb {
            background
        } else {
            background.map(|c| c.powf(2.2))
        };
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &msaa,
                    depth_slice: None,
                    resolve_target: Some(&target),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: bg[0],
                            g: bg[1],
                            b: bg[2],
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            for (pipeline, buffer) in [
                (&self.mesh_pipeline, &self.mesh),
                (&self.grid_pipeline, &self.layers[Layer::Grid as usize]),
                (&self.overlay_pipeline, &self.layers[Layer::Ghost as usize]),
                (&self.grid_pipeline, &self.layers[Layer::Dims as usize]),
                (
                    &self.overlay_pipeline,
                    &self.layers[Layer::Overlay as usize],
                ),
            ] {
                if let Some((b, n)) = buffer {
                    pass.set_pipeline(pipeline);
                    pass.set_vertex_buffer(0, b.slice(..));
                    pass.draw(0..*n, 0..1);
                }
            }
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
    }
}

/// "Nice" grid cell sizes (mm), so the board-plane grid reads like graph
/// paper: the one giving about 15 cells across three times the model's
/// footprint.
pub fn grid_step(footprint: f32) -> f32 {
    let span = footprint * 3.0;
    [
        0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0,
    ]
    .into_iter()
    .min_by(|a: &f32, b: &f32| ((span / a - 15.0).abs()).total_cmp(&(span / b - 15.0).abs()))
    .unwrap_or(1.0)
}

/// The grid on the board plane (z = 0) around `center`.
pub fn grid_lines(center: Vec2, footprint: f32) -> Vec<Line> {
    let step = grid_step(footprint);
    let n = ((footprint * 3.0 / step / 2.0).round() as i32).max(1);
    let c = Vec2::new(
        (center.x / step).round() * step,
        (center.y / step).round() * step,
    );
    let half = n as f32 * step;
    let mut lines = Vec::new();
    for i in -n..=n {
        let o = i as f32 * step;
        let major = i == 0 || (c.x + o).abs() < step * 0.01;
        let color = if major {
            [0.45, 0.47, 0.52, 0.7]
        } else {
            [0.55, 0.57, 0.62, 0.35]
        };
        lines.push(Line {
            a: Vec3::new(c.x + o, c.y - half, 0.0),
            b: Vec3::new(c.x + o, c.y + half, 0.0),
            color,
            width: 1.0,
        });
        let major = i == 0 || (c.y + o).abs() < step * 0.01;
        let color = if major {
            [0.45, 0.47, 0.52, 0.7]
        } else {
            [0.55, 0.57, 0.62, 0.35]
        };
        lines.push(Line {
            a: Vec3::new(c.x - half, c.y + o, 0.0),
            b: Vec3::new(c.x + half, c.y + o, 0.0),
            color,
            width: 1.0,
        });
    }
    lines
}
