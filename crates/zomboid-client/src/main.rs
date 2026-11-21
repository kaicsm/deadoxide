//! Zomboid-rs client. `zomboid-client <root> [cell] [--shot out.png [cx cy zoom w h]]`
mod light;
mod stream_gpu;
use std::collections::BTreeMap;
use scene::*;
use sim::{Player, World, TICK_HZ};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
use zomboid_scene::{scene, sim};
use zomboid_scene::async_stream::{AsyncWorldStreamer, Completion};

/// Measured against the real game (tests/golden/spawn_rosewood): camera centre sits 56 px above the player feet at zoom 1. One sample only.
const CAM_OVER_FEET: f32 = 56.0;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CamU {
    center: [f32; 2],
    scale: [f32; 2],
    /// x = ambient multiplier on the world colour (1.0 = unlit), yzw unused
    light: [f32; 4],
}

#[derive(Clone, Copy)]
struct Camera {
    center: [f32; 2],
    zoom: f32,
    max_level: usize,
    ambient: f32,
}

struct Gfx {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    bind_layout: wgpu::BindGroupLayout,
    atlas_sampler: wgpu::Sampler,
    cam_buf: wgpu::Buffer,
    bands: BTreeMap<(i32, i32), GpuInstanceBand>,
    min_level: i32,
    atlas_tex: wgpu::Texture,
    player_buf: wgpu::Buffer,
    clear: [f64; 3],
}

struct GpuInstanceBand {
    buffer: wgpu::Buffer,
    capacity: usize,
    count: u32,
    /// CPU shadow: do not upload untouched bands after a new chunk is integrated.
    bytes: Vec<u8>,
}

impl Gfx {
    /// Replace the atlas for an initial snapshot or a non-append-only change.
    fn set_atlas(&mut self, scene: &Scene) {
        let a = &scene.atlas;
        // Reuse the allocated texture and bind group if a repack retains its extent.
        // This avoids an extra GPU allocation and pipeline rebind on such updates.
        let extent = self.atlas_tex.size();
        if extent.width == a.width && extent.height == a.height
            && extent.depth_or_array_layers == a.layers {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas_tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &a.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(a.width * 4),
                    rows_per_image: Some(a.height),
                },
                extent,
            );
            return;
        }
        let tex = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("stream atlas"),
                size: wgpu::Extent3d { width: a.width, height: a.height, depth_or_array_layers: a.layers },
                mip_level_count: 1, sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor, &a.rgba,
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default()
        });
        self.bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stream atlas bind"), layout: &self.bind_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.cam_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.atlas_sampler) },
            ],
        });
        self.atlas_tex = tex;
    }
    /// Growing an append-only atlas must not re-upload its previously known
    /// pixels from the CPU. Copy them on the GPU and upload only new rectangles.
    /// Existing sprite placements remain stable in the streaming atlas.
    fn grow_atlas_and_patch(&mut self, scene: &Scene) {
        let old = self.atlas_tex.size();
        let a = &scene.atlas;
        if old.width == a.width && old.height == a.height
            && old.depth_or_array_layers == a.layers
        {
            self.patch_atlas(scene);
            return;
        }
        if a.width < old.width || a.height < old.height
            || a.layers < old.depth_or_array_layers
        {
            // A shrinking/non-append-only atlas needs a complete replacement.
            self.set_atlas(scene);
            return;
        }
        let grown = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("grown stream atlas"),
            size: wgpu::Extent3d {
                width: a.width, height: a.height, depth_or_array_layers: a.layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("grow stream atlas copy"),
        });
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_tex, mip_level: 0,
                origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &grown, mip_level: 0,
                origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All,
            },
            old,
        );
        self.queue.submit([encoder.finish()]);
        let view = grown.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        self.bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grown stream atlas bind"), layout: &self.bind_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.cam_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.atlas_sampler) },
            ],
        });
        self.atlas_tex = grown;
        self.patch_atlas(scene);
    }

    /// Upload only the new rectangles appended by the World Streamer.
    fn patch_atlas(&mut self, scene: &Scene) {
        for patch in &scene.gpu_atlas_patches {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas_tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: patch.x, y: patch.y, z: patch.layer },
                    aspect: wgpu::TextureAspect::All,
                },
                &patch.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(patch.width * 4),
                    rows_per_image: Some(patch.height),
                },
                wgpu::Extent3d { width: patch.width, height: patch.height,
                    depth_or_array_layers: 1 },
            );
        }
    }

    /// Keep the exact (z, y, x) painter order with separate GPU buffers for
    /// eight-row bands. IsoChunkMap.doLoadGridsquare only changes local squares;
    /// wgpu likewise need not receive the entire 9x9 window for every chunk.
    fn set_instances(&mut self, scene: &Scene) {
        let mut next = BTreeMap::new();
        let mut uploaded = 0usize;
        let mut unchanged = 0usize;
        let item_size = std::mem::size_of::<Instance>();
        for slice in stream_gpu::partition_bands(&scene.meta) {
            let key = (slice.z, slice.y_band);
            let bytes: &[u8] = bytemuck::cast_slice(&scene.instances[slice.start..slice.end]);
            let count = (slice.end - slice.start) as u32;
            let mut band = if let Some(band) = self.bands.remove(&key) {
                band
            } else {
                let capacity = bytes.len().next_power_of_two().max(item_size);
                GpuInstanceBand {
                    buffer: self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("stream instance band"),
                        size: capacity as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }),
                    capacity,
                    count: 0,
                    bytes: Vec::new(),
                }
            };
            if bytes.len() > band.capacity {
                band.capacity = bytes.len().next_power_of_two();
                band.buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("stream instance band grow"),
                    size: band.capacity as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                band.bytes.clear();
            }
            if band.bytes.as_slice() != bytes {
                // The insertion/removal of squares can shift following sprites
                // within this band. Upload its new contiguous image, not all bands.
                if !bytes.is_empty() {
                    self.queue.write_buffer(&band.buffer, 0, bytes);
                    uploaded += bytes.len();
                }
                band.bytes.clear();
                band.bytes.extend_from_slice(bytes);
            } else {
                unchanged += 1;
            }
            band.count = count;
            next.insert(key, band);
        }
        self.bands = next; // outgoing bands are freed with their buffers
        self.min_level = scene.min_level;
        log::trace!("World Streamer GPU instances: {} bands, {} unchanged, {} bytes uploaded",
            self.bands.len(), unchanged, uploaded);
    }
    fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        scene: &Scene,
    ) -> Self {
        let a = &scene.atlas;
        let tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("atlas"),
                size: wgpu::Extent3d {
                    width: a.width,
                    height: a.height,
                    depth_or_array_layers: a.layers,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &a.rgba,
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let samp = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let cam_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cam"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cam_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&samp),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprites"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 36,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attrs,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let player_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("player"),
            size: (std::mem::size_of::<Instance>() * 2048) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut gfx = Self {
            device,
            queue,
            pipeline,
            bind,
            bind_layout: bgl,
            atlas_sampler: samp,
            cam_buf,
            bands: BTreeMap::new(),
            min_level: scene.min_level,
            atlas_tex: tex,
            player_buf,
            clear: scene.clear.map(|c| c as f64),
        };
        gfx.set_instances(scene);
        gfx
    }

    fn draw(&self, view: &wgpu::TextureView, size: (u32, u32), cam: Camera, markers: &[Instance]) {
        let markers = &markers[..markers.len().min(2048)];
        if !markers.is_empty() {
            self.queue
                .write_buffer(&self.player_buf, 0, bytemuck::cast_slice(markers));
        }
        let u = CamU {
            center: cam.center,
            scale: [
                2.0 * cam.zoom / size.0 as f32,
                2.0 * cam.zoom / size.1 as f32,
            ],
            light: [cam.ambient, 0.0, 0.0, 0.0],
        };
        self.queue
            .write_buffer(&self.cam_buf, 0, bytemuck::bytes_of(&u));
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.clear[0],
                            g: self.clear[1],
                            b: self.clear[2],
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            let max_z = self.min_level.saturating_add(cam.max_level as i32);
            // BTreeMap iterates in (z, 8-row band) order, exactly preserving
            // the original flat (z, y, x) painter order and alpha blending.
            for (&(z, _), band) in &self.bands {
                if z > max_z { break; }
                pass.set_vertex_buffer(0, band.buffer.slice(..));
                pass.draw(0..4, 0..band.count);
            }
            if !markers.is_empty() {
                pass.set_vertex_buffer(0, self.player_buf.slice(..));
                pass.draw(0..4, 0..markers.len() as u32);
            }
        }
        self.queue.submit([enc.finish()]);
    }
}

fn init_device(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'static>>,
) -> anyhow::Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue)> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: surface,
        ..Default::default()
    }))?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
    Ok((adapter, device, queue))
}

/// Offscreen render + readback to PNG (same draw path as the window).
fn screenshot(
    scene: &Scene,
    cam: Camera,
    size: (u32, u32),
    player: Option<(f32, f32, i32)>,
    extra: Vec<Instance>,
    out: &str,
) -> anyhow::Result<()> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let (_a, device, queue) = init_device(&instance, None)?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let gfx = Gfx::new(device, queue, format, scene);
    let extent = wgpu::Extent3d {
        width: size.0,
        height: size.1,
        depth_or_array_layers: 1,
    };
    let tex = gfx.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    let mut mk: Vec<Instance> = extra;
    mk.extend(player.map(|(x, y, z)| scene.marker("__player", x, y, z)));
    gfx.draw(&view, size, cam, &mk);
    let bpr = (size.0 * 4).div_ceil(256) * 256;
    let buf = gfx.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (bpr * size.1) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gfx.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        tex.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bpr),
                rows_per_image: Some(size.1),
            },
        },
        extent,
    );
    gfx.queue.submit([enc.finish()]);
    let slice = buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    gfx.device.poll(wgpu::PollType::wait_indefinitely())?;
    let data = slice.get_mapped_range()?;
    let mut rgba = Vec::with_capacity((size.0 * size.1 * 4) as usize);
    for row in 0..size.1 {
        rgba.extend_from_slice(&data[(row * bpr) as usize..(row * bpr + size.0 * 4) as usize]);
    }
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(std::fs::File::create(out)?),
        size.0,
        size.1,
    );
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&rgba)?;
    println!("screenshot -> {out}");
    Ok(())
}

struct Live {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gfx: Gfx,
}

struct App {
    scene: Scene,
    async_stream: Option<AsyncWorldStreamer>,
    live: Option<Live>,
    cam: Camera,
    world: World,
    lua: zomboid_lua::events::EventRuntime,
    acc: f32,
    keys: std::collections::HashSet<KeyCode>,
    drag: Option<(f64, f64)>,
    last: std::time::Instant,
    /// Continuous height of the player (fractional on stairs); `None` until the first move.
    height: Option<f32>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.live.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("zomboid-rs")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));
        let window = Arc::new(el.create_window(attrs).unwrap());
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone()).unwrap();
        let (adapter, device, queue) = init_device(&instance, Some(&surface)).expect("gpu");
        let caps = surface.get_capabilities(&adapter);
        // non-sRGB so atlas bytes pass through unchanged (matches the CPU reference)
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let sz = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: sz.width.max(1),
            height: sz.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: Default::default(),
        };
        surface.configure(&device, &config);
        let gfx = Gfx::new(device, queue, format, &self.scene);
        self.live = Some(Live {
            window,
            surface,
            config,
            gfx,
        });
        self.last = std::time::Instant::now();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, ev: WindowEvent) {
        let Some(l) = &mut self.live else { return };
        match ev {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) if s.width > 0 && s.height > 0 => {
                l.config.width = s.width;
                l.config.height = s.height;
                l.surface.configure(&l.gfx.device, &l.config);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(k) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        self.keys.insert(k);
                        let top = self.scene.level_ranges.len() - 2;
                        match k {
                            KeyCode::Escape => el.exit(),
                            KeyCode::PageUp => {
                                self.cam.max_level = (self.cam.max_level + 1).min(top)
                            }
                            KeyCode::KeyE if !event.repeat => {
                                let p = self.world.player;
                                if let Some(worker) = &self.async_stream {
                                    if let Err(e) = worker.toggle_door(p.x, p.y, p.z) {
                                        log::error!("async door request: {e}");
                                    }
                                } else if self.scene.toggle_door_near(p.x, p.y, p.z) {
                                    if let Some(g) = self.live.as_mut().map(|l| &mut l.gfx) {
                                        g.set_instances(&self.scene);
                                    }
                                }
                            }
                            KeyCode::PageDown => {
                                self.cam.max_level = self.cam.max_level.saturating_sub(1)
                            }
                            _ => {}
                        }
                    } else {
                        self.keys.remove(&k);
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                self.cam.zoom = (self.cam.zoom * 1.1f32.powf(y)).clamp(0.05, 4.0);
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                self.drag = if state == ElementState::Pressed {
                    Some((f64::NAN, 0.0))
                } else {
                    None
                };
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(d) = &mut self.drag {
                    if !d.0.is_nan() {
                        self.cam.center[0] -= (position.x - d.0) as f32 / self.cam.zoom;
                        self.cam.center[1] -= (position.y - d.1) as f32 / self.cam.zoom;
                    }
                    *d = (position.x, position.y);
                }
            }
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                let dt = (now - self.last).as_secs_f32();
                self.last = now;
                // WASD = screen directions; world axes are rotated 45 degrees in the isometric view
                let (mut sx, mut sy) = (0.0f32, 0.0f32);
                for k in &self.keys {
                    match k {
                        KeyCode::KeyW | KeyCode::ArrowUp => {
                            sx -= 1.0;
                            sy -= 1.0;
                        }
                        KeyCode::KeyS | KeyCode::ArrowDown => {
                            sx += 1.0;
                            sy += 1.0;
                        }
                        KeyCode::KeyA | KeyCode::ArrowLeft => {
                            sx -= 1.0;
                            sy += 1.0;
                        }
                        KeyCode::KeyD | KeyCode::ArrowRight => {
                            sx += 1.0;
                            sy -= 1.0;
                        }
                        _ => {}
                    }
                }
                // fixed-step simulation (30 Hz), render interpolation not implemented
                self.acc += dt.min(0.25);
                while self.acc >= 1.0 / TICK_HZ {
                    // The player moves through the scene's ported collision and stairs (4 squares per second is a
                    // placeholder speed); the world step then only advances the clock and the placeholder zombies.
                    let len = (sx * sx + sy * sy).sqrt();
                    if len > 0.0 {
                        let p = self.world.player;
                        let mut m = Mover {
                            x: p.x,
                            y: p.y,
                            z: self.height.unwrap_or(p.z as f32),
                        };
                        let d = 4.0 / TICK_HZ;
                        let (dx, dy) = (sx / len * d, sy / len * d);
                        // The original IsoMovingObject.DoCollide cannot walk into
                        // a null grid square. Keep that safety rule, but queue the
                        // absent chunk ahead of an imminent movement/collision.
                        if let Some((wx, wy)) = self.scene.stream_pending_move_chunk(&m, dx, dy) {
                            if let Some(worker) = &mut self.async_stream {
                                match worker.prioritize_chunk(wx, wy, (m.x, m.y)) {
                                    Ok(true) => log::debug!(
                                        "World Streamer collision waiting for chunk {wx},{wy} near {:.2},{:.2}",
                                        m.x, m.y),
                                    Ok(false) => {},
                                    Err(e) => log::error!("stream movement priority: {e}"),
                                }
                            }
                        }
                        self.scene.move_character(&mut m, dx, dy);
                        self.height = Some(m.z);
                        let floor = m.z.floor() as i32;
                        if floor != p.z {
                            // show the floor the player is on (level index = z - min_level)
                            let top = self.scene.level_ranges.len() - 2;
                            self.cam.max_level =
                                ((floor - self.scene.min_level).max(0) as usize).min(top);
                        }
                        self.world.player = Player {
                            x: m.x,
                            y: m.y,
                            z: floor,
                        };
                    }
                    if let Err(error) = advance_world(
                        &mut self.world,
                        &self.scene.collision,
                        (0.0, 0.0),
                        &self.lua,
                    ) {
                        log::error!("Lua event dispatch failed: {error}");
                    }
                    self.acc -= 1.0 / TICK_HZ;
                }
                // IsoChunkMap.update: move the residency window when the player
                // changes chunk. The window itself already includes surrounding
                // chunks, so an extra hard-coded eight-tile lookahead is not Java's
                // pedestrian loading policy.
                if let Some(worker) = &mut self.async_stream {
                    if let Err(e) = worker.request(self.world.player.x, self.world.player.y) {
                        log::error!("stream request: {e}");
                    }
                }
                // IsoChunkMap.updateInternal incorporates 1 + ready * 3 / width
                // completed chunks, in FIFO order, not a constant two-chunk batch.
                let completed = self.async_stream.as_mut()
                    .map(AsyncWorldStreamer::poll).unwrap_or_default();
                let mut atlas_dirty = false;
                let mut instances_dirty = false;
                let mut stream_failed = false;
                for completion in completed {
                    match completion {
                        Completion::Prepared(snapshot) => {
                            let commit_start = std::time::Instant::now();
                            match self.scene.accept_stream_snapshot(snapshot) {
                                Ok(report) => {
                                    atlas_dirty |= report.atlas_changed;
                                    instances_dirty = true;
                                    log::debug!("World Streamer: +{} -{} disk {} teleport {} atlas {} commit {:.1}ms",
                                        report.loaded, report.unloaded, report.restored_from_disk,
                                        report.teleported, report.atlas_changed,
                                        commit_start.elapsed().as_secs_f64() * 1000.0);
                                }
                                Err(e) => {
                                    log::error!("stream merge: {e}; reverting to synchronous streaming");
                                    stream_failed = true;
                                    break;
                                }
                            }
                        }
                        Completion::Failed(e) => {
                            log::error!("async streaming failed: {e}; reverting to synchronous streaming");
                            stream_failed = true;
                            break;
                        }
                    }
                }
                if stream_failed { self.async_stream = None; }
                let gpu_update_started = std::time::Instant::now();
                if instances_dirty {
                    // Upload once per render frame even when the Java consumption
                    // quota incorporates several completed chunks.
                    if atlas_dirty {
                        if self.scene.gpu_atlas_full {
                            if self.scene.gpu_atlas_patches.is_empty() {
                                // Initial worker snapshot may contain a different
                                // full atlas; copying the previous one is not safe.
                                l.gfx.set_atlas(&self.scene);
                            } else {
                                l.gfx.grow_atlas_and_patch(&self.scene);
                            }
                        } else { l.gfx.patch_atlas(&self.scene); }
                        self.scene.gpu_atlas_full = false;
                        self.scene.gpu_atlas_patches.clear();
                    }
                    l.gfx.set_instances(&self.scene);
                    let top = self.scene.level_ranges.len().saturating_sub(2);
                    self.cam.max_level = ((self.world.player.z - self.scene.min_level).max(0) as usize).min(top);
                }
                let gpu_update_ms = gpu_update_started.elapsed().as_secs_f64() * 1000.0;
                if gpu_update_ms > 16.0 {
                    log::warn!("World Streamer render upload {:.1}ms (atlas_changed={}, instances_changed={})",
                        gpu_update_ms, atlas_dirty, instances_dirty);
                }
                if self.async_stream.is_none() {
                    // Explicit --no-async-stream, or a worker initialization error.
                    match self.scene.stream_around(self.world.player.x, self.world.player.y) {
                        Ok(Some(report)) => {
                            if report.atlas_changed { l.gfx.set_atlas(&self.scene); }
                            l.gfx.set_instances(&self.scene);
                        }
                        Ok(None) => {}
                        Err(e) => log::error!("synchronous stream fallback: {e}"),
                    }
                }
                {
                    let c = iso(
                        self.world.player.x,
                        self.world.player.y,
                        self.world.player.z,
                    );
                    self.cam.center = [c[0], c[1] - CAM_OVER_FEET];
                }
                let mut markers: Vec<Instance> = self
                    .world
                    .zombies
                    .iter()
                    .map(|z| self.scene.marker("__zombie", z.x, z.y, self.world.player.z))
                    .collect();
                markers.push(self.scene.marker(
                    "__player",
                    self.world.player.x,
                    self.world.player.y,
                    self.world.player.z,
                ));
                use wgpu::CurrentSurfaceTexture as T;
                match l.surface.get_current_texture() {
                    T::Success(f) | T::Suboptimal(f) => {
                        let view = f.texture.create_view(&Default::default());
                        l.gfx
                            .draw(&view, (l.config.width, l.config.height), self.cam, &markers);
                        l.gfx.queue.present(f);
                    }
                    T::Outdated | T::Lost => l.surface.configure(&l.gfx.device, &l.config),
                    _ => {}
                }
                let frame_ms = now.elapsed().as_secs_f64() * 1000.0;
                if frame_ms > 33.0 {
                    log::warn!("World Streamer frame {:.1}ms (render upload {:.1}ms, player {:.2},{:.2})",
                        frame_ms, gpu_update_ms, self.world.player.x, self.world.player.y);
                }
                l.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let a: Vec<String> = std::env::args().collect();
    let root = std::path::PathBuf::from(a.get(1).map(String::as_str).unwrap_or(".."));
    // --save <save folder or players.db>: read a consistent committed player row, including WAL.
    let saved = a
        .iter()
        .position(|s| s == "--save")
        .map(|j| load_saved_player(&a[j + 1]))
        .transpose()?;
    let mut saved_time = None;
    let mut saved_day = None;
    if let Some(j) = a.iter().position(|s| s == "--save") {
        let save_path = std::path::Path::new(&a[j + 1]);
        let folder = if save_path.is_dir() {
            Some(save_path)
        } else {
            save_path.parent()
        };
        if let Some(path) = folder
            .map(|p| p.join("map_sand.bin"))
            .filter(|p| p.is_file())
        {
            match zomboid_content::sandbox::load_day_length(&path) {
                Ok(day) => {
                    println!(
                        "save day length: {} minutes (option {})",
                        day.minutes, day.option
                    );
                    saved_day = Some(day);
                }
                Err(e) => eprintln!("save day length unavailable: {e}"),
            }
        }
        if let Some(path) = folder.map(|p| p.join("map_t.bin")).filter(|p| p.is_file()) {
            match zomboid_content::save::load_game_time(&path) {
                Ok(t) => {
                    println!(
                        "save time: {:02}:{:02}, year {}, month index {}, day index {}",
                        t.hour(),
                        t.minutes(),
                        t.year,
                        t.month_index,
                        t.day_index
                    );
                    saved_time = Some(t);
                }
                Err(e) => eprintln!("save time unavailable: {e}"),
            }
        }
    }
    let cell = a
        .get(2)
        .filter(|s| !s.starts_with("--"))
        .cloned()
        .unwrap_or_else(|| match &saved {
            Some(p) => {
                let (cx, cy) = zomboid_content::save::cell_of(p.x, p.y);
                format!("{cx}_{cy}")
            }
            None => "41_39".into(),
        });
    let (r0, r1) = cell
        .split_once(':')
        .unwrap_or((cell.as_str(), cell.as_str()));
    let pc = |s: &str| {
        let (x, y) = s.split_once('_').expect("cell like 41_39");
        (x.parse::<i32>().unwrap(), y.parse::<i32>().unwrap())
    };
    let ((x0, y0), (x1, y1)) = (pc(r0), pc(r1));
    let t0 = std::time::Instant::now();
    // --save: chunks the save holds come from its map/*.bin (IsoChunk.LoadOrCreate), the rest from the map.
    let save_dir = a.iter().position(|s| s == "--save").map(|j| {
        let p = std::path::Path::new(&a[j + 1]);
        if p.is_dir() {
            p.to_path_buf()
        } else {
            p.parent().unwrap_or(p).to_path_buf()
        }
    });
    let persist_worldgen = a.iter().any(|s| s == "--persist-worldgen");
    if persist_worldgen && save_dir.is_none() {
        return Err(anyhow::anyhow!("--persist-worldgen requires --save <existing save folder>"));
    }
    // Stream by default in the interactive client. --shot keeps the old deterministic
    // full-cell scene for screenshot oracles; --no-stream opts out for debugging.
    let streaming = !a.iter().any(|s| s == "--shot" || s == "--no-stream");
    let stream_radius: i32 = a.iter().position(|s| s == "--stream-radius")
        .and_then(|i| a.get(i + 1)).map(|s| s.parse())
        .transpose().map_err(|e| anyhow::anyhow!("--stream-radius: {e}"))?.unwrap_or(4);
    let stream_player = a.iter().position(|s| s == "--player")
        .and_then(|i| a.get(i + 1).zip(a.get(i + 2)))
        .map(|(x, y)| Ok::<_, anyhow::Error>((x.parse::<f32>()?, y.parse::<f32>()?)))
        .transpose()?
        .or_else(|| saved.as_ref().map(|p| (p.x as f32, p.y as f32)))
        .unwrap_or(((x0 * 256 + 116) as f32, (y0 * 256 + 116) as f32));
    // Java creates its World Streamer before the player begins moving. Start
    // the isolated Rust loading context while the foreground's initial region
    // is being prepared, instead of making the first walk wait for another
    // entire synchronous world initialization after the window appears.
    let mut async_stream = if streaming && !persist_worldgen
        && !a.iter().any(|s| s == "--no-async-stream" || s == "--headless-ticks") {
        Some(AsyncWorldStreamer::spawn(
            root.clone(), save_dir.clone(), stream_player, stream_radius,
            persist_worldgen,
        ).map_err(anyhow::Error::msg)?)
    } else { None };
    let mut scene = if streaming {
        scene::load_streaming(&root, save_dir.as_deref(), stream_player.0, stream_player.1,
            stream_radius, persist_worldgen)?
    } else if persist_worldgen {
        scene::load_region_from_save_persist(&root, save_dir.as_deref()
            .ok_or_else(|| anyhow::anyhow!("missing --save directory"))?, x0, y0, x1, y1)?
    } else {
        scene::load_region_from_save(&root, save_dir.as_deref(), x0, y0, x1, y1)?
    };
    if let Some(st) = &scene.state {
        if save_dir.is_some() {
            println!(
                "save map: {} chunks loaded from the save",
                st.loaded_from_save
            );
        }
    }
    println!(
        "cell {cell}: {} sprites, atlas {}x{}, unresolved {}, levels {}, loaded in {:.2}s",
        scene.instances.len(),
        scene.atlas.width,
        scene.atlas.height,
        scene.unresolved,
        scene.level_ranges.len() - 1,
        t0.elapsed().as_secs_f32()
    );
    let top = scene.level_ranges.len() - 2;
    let (gx, gy) = if streaming { stream_player } else {
        ((x0 * 256 + 116) as f32, (y0 * 256 + 116) as f32)
    };
    let mut cam = Camera {
        center: [(gx - gy) * 64.0, (gx + gy) * 32.0],
        zoom: 0.5,
        max_level: top,
        ambient: 1.0,
    };
    if let Some(p) = &saved {
        let position = iso(p.x as f32, p.y as f32, p.z);
        cam.center = [position[0], position[1] - CAM_OVER_FEET];
        cam.max_level = (p.z - scene.min_level).clamp(0, top as i32) as usize;
    }
    if let Some(i) = a.iter().position(|s| s == "--shot") {
        let f = |n: usize, d: f32| a.get(i + n).and_then(|s| s.parse().ok()).unwrap_or(d);
        cam.center = [f(2, cam.center[0]), f(3, cam.center[1])];
        cam.zoom = f(4, 0.5);
        if let Some(j) = a.iter().position(|s| s == "--level") {
            cam.max_level = a[j + 1].parse().unwrap();
        }
        let pl: Option<(f32, f32, i32)> = a
            .iter()
            .position(|s| s == "--player")
            .map(|j| {
                let z = a
                    .iter()
                    .position(|s| s == "--player-z")
                    .and_then(|k| a.get(k + 1))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                (a[j + 1].parse().unwrap(), a[j + 2].parse().unwrap(), z)
            })
            .or_else(|| saved.as_ref().map(|p| (p.x as f32, p.y as f32, p.z)));
        // Night ambient on lit squares: game/port luma ratio is ~0.8 in the room interior on two real frames (docs/COMPATIBILITY.md, tools/ratio_map.mjs).
        if a.iter().any(|s| s == "--lit-room") {
            cam.ambient = 0.8;
        }
        if let Some(j) = a.iter().position(|s| s == "--ambient") {
            cam.ambient = a[j + 1].parse().unwrap_or(1.0);
        }
        if a.iter().any(|s| s == "--lit-room") {
            if let Some((px, py, pz)) = pl {
                println!(
                    "lit-room: {:?}, instances now {}",
                    scene.retain_rooms(
                        px,
                        py,
                        pz,
                        a.iter().any(|s| s == "--probe-rooms"),
                        a.iter()
                            .position(|s| s == "--los")
                            .and_then(|j| a[j + 1].parse().ok())
                    ),
                    scene.instances.len()
                );
            }
        }
        // --zones <Type>: outline that zone type from the map's objects.lua (debug overlay, tile box = 60x60 around the player/camera)
        let mut extra = vec![];
        if let Some(j) = a.iter().position(|s| s == "--zones") {
            let zs = zomboid_content::zones::parse_objects(&std::fs::read(
                root.join("media/maps/Muldraugh, KY/objects.lua"),
            )?)
            .map_err(|e| anyhow::anyhow!(e))?;
            let (px, py, pz) = pl.unwrap_or((gx, gy, 0));
            extra = scene.zone_dots(
                "__player",
                &zs,
                &a[j + 1],
                (
                    (px - 60.0) as i32,
                    (py - 60.0) as i32,
                    (px + 60.0) as i32,
                    (py + 60.0) as i32,
                ),
                pz,
                1.0,
            );
            println!("zone overlay '{}': {} dots", a[j + 1], extra.len());
        }
        return screenshot(
            &scene,
            cam,
            (f(5, 1280.0) as u32, f(6, 720.0) as u32),
            pl,
            extra,
            a.get(i + 1).map(String::as_str).unwrap_or("shot.png"),
        );
    }
    let player = if streaming && a.iter().any(|v| v == "--player") {
        Player { x: gx, y: gy, z: saved.as_ref().map_or(0, |p| p.z) }
    } else { match &saved {
        Some(p) => {
            println!(
                "save: {} at ({:.1}, {:.1}) z{}{}",
                p.name,
                p.x,
                p.y,
                p.z,
                if p.dead { " (dead)" } else { "" }
            );
            Player {
                x: p.x as f32,
                y: p.y as f32,
                z: p.z,
            }
        }
        None => spawn(&scene, gx, gy),
    }};
    if streaming {
        let position = iso(player.x, player.y, player.z);
        cam.center = [position[0], position[1] - CAM_OVER_FEET];
        cam.max_level = (player.z - scene.min_level).clamp(0, top as i32) as usize;
    }
    let mut world = World::new(1, player, &scene.collision, 40, 20);
    if let (Some(time), Some(day)) = (&saved_time, &saved_day) {
        world.clock = sim::GameClock::from_save(time, day);
        if world.clock.is_some() {
            println!("clock: experimental normal speed; Lua event dispatch enabled; weather and speed controls not connected");
        } else {
            eprintln!("save clock unavailable: unsupported date");
        }
    }
    let lua = zomboid_lua::events::EventRuntime::new()
        .map_err(|e| anyhow::anyhow!("Lua initialization: {e}"))?;
    // An existing save can provide the original persistent fishing trash seed.
    // No fish-school generation or seed updates are implied by this limited API.
    let fishing_path = if let Some(i) = a.iter().position(|s| s == "--fishing-data") {
        Some(std::path::PathBuf::from(a.get(i + 1).ok_or_else(|| {
            anyhow::anyhow!("--fishing-data requires a path")
        })?))
    } else if let Some(i) = a.iter().position(|s| s == "--save") {
        let path = std::path::Path::new(&a[i + 1]);
        let directory = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        let candidate = directory.join("fishingData.bin");
        candidate.is_file().then_some(candidate)
    } else {
        None
    };
    if let Some(path) = fishing_path {
        let saved_fishing = zomboid_lua::fishing::read_fishing_save(&path)?;
        lua.install_fishing_save(&saved_fishing)
            .map_err(|e| anyhow::anyhow!("fishing Lua API: {e}"))?;
        println!("fishing: loaded saved seeds and chum; fish abundance and other fishing APIs incomplete");
    }
    if let Some(i) = a.iter().position(|s| s == "--save") {
        let path = std::path::Path::new(&a[i + 1]);
        let directory = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        let zone_path = directory.join("map_zone.bin");
        let sand_path = directory.join("map_sand.bin");
        if zone_path.is_file() && sand_path.is_file() {
            let load = || -> anyhow::Result<zomboid_lua::fishing::FishPopulation> {
                let zones = zomboid_content::fishing_zones::parse_fishing_zones(&std::fs::read(
                    &zone_path,
                )?)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
                let abundance =
                    zomboid_content::sandbox::parse_fish_abundance(&std::fs::read(&sand_path)?)
                        .map_err(|e| anyhow::anyhow!("{e}"))?;
                Ok(zomboid_lua::fishing::FishPopulation {
                    abundance,
                    zones: zones.zones,
                })
            };
            match load() {
                Ok(population) => match lua.set_fishing_population(population) {
                    Ok(()) => println!("fishing: loaded saved fish abundance and rectangle zones"),
                    Err(e) => eprintln!("fishing population unavailable: {e}"),
                },
                Err(e) => eprintln!("fishing population unavailable: {e}"),
            }
        }
    }
    if let Some(i) = a.iter().position(|s| s == "--rng-seed") {
        let value = a
            .get(i + 1)
            .ok_or_else(|| anyhow::anyhow!("--rng-seed requires eight hexadecimal digits"))?;
        if value.len() != 8 {
            anyhow::bail!("--rng-seed requires eight hexadecimal digits");
        }
        let seed = u32::from_str_radix(value, 16)?;
        let rule = zomboid_content::rng::load_rule(&root)?;
        lua.set_fishing_rng(zomboid_content::rng::CellularRng::new(
            seed.to_be_bytes(),
            rule,
        ))
        .map_err(|e| anyhow::anyhow!("fishing RNG: {e}"))?;
        println!(
            "fishing: controlled RNG seed installed; global game RNG integration is incomplete"
        );
    }
    lua.update_fishing_time(
        world
            .clock
            .as_ref()
            .and_then(|clock| clock.calendar_minutes(&chrono::Local)),
    );
    if let Some(i) = a.iter().position(|s| s == "--lua-script") {
        let path = a
            .get(i + 1)
            .ok_or_else(|| anyhow::anyhow!("--lua-script requires a path"))?;
        lua.load_file(std::path::Path::new(path))
            .map_err(|e| anyhow::anyhow!("Lua script {path}: {e}"))?;
    }
    if let Some(i) = a.iter().position(|s| s == "--headless-ticks") {
        let ticks: u32 = a
            .get(i + 1)
            .ok_or_else(|| anyhow::anyhow!("--headless-ticks requires a count"))?
            .parse()?;
        let trace_events = a.iter().any(|arg| arg == "--trace-clock-events");
        for _ in 0..ticks {
            advance_world(&mut world, &scene.collision, (0.0, 0.0), &lua)?;
            if trace_events {
                for event in &world.clock_events {
                    println!("clock event tick={} name={}", world.tick, event.name());
                }
            }
        }
        if let Some(clock) = &world.clock {
            println!(
                "clock after {ticks} ticks: bits={:08x} year={} month_index={} day_index={} nights={}",
                clock.time.to_bits(),
                clock.year,
                clock.month,
                clock.day,
                clock.nights_survived
            );
        } else {
            println!("clock after {ticks} ticks: unavailable (save time and sandbox day length required)");
        }
        return Ok(());
    }
    println!(
        "world: seed 1, {} placeholder zombies, tick {} Hz",
        world.zombies.len(),
        TICK_HZ
    );
    // When persisting new chunks, finish the foreground's initial save writes
    // before starting the second loader. Two writers must never race on map/*.bin.
    if streaming && persist_worldgen && !a.iter().any(|s| s == "--no-async-stream") {
        async_stream = Some(AsyncWorldStreamer::spawn(
            root.clone(), save_dir.clone(), stream_player, stream_radius,
            persist_worldgen,
        ).map_err(anyhow::Error::msg)?);
    }
    let el = EventLoop::new()?;
    el.set_control_flow(ControlFlow::Poll);
    el.run_app(&mut App {
        scene,
        async_stream,
        live: None,
        cam,
        world,
        lua,
        acc: 0.0,
        keys: Default::default(),
        drag: None,
        last: std::time::Instant::now(),
        height: None,
    })?;
    Ok(())
}

/// Dispatch each step immediately; later steps overwrite the world's signal buffer.
fn advance_world(
    world: &mut World,
    collision: &sim::Collision,
    input: (f32, f32),
    lua: &zomboid_lua::events::EventRuntime,
) -> anyhow::Result<()> {
    world.step(collision, input);
    lua.update_fishing_time(
        world
            .clock
            .as_ref()
            .and_then(|clock| clock.calendar_minutes(&chrono::Local)),
    );
    for event in &world.clock_events {
        for error in lua
            .dispatch(event.name())
            .map_err(|e| anyhow::anyhow!("Lua event {}: {e}", event.name()))?
        {
            log::error!("Lua event {} callback: {}", error.event, error.message);
        }
    }
    Ok(())
}

#[cfg(test)]
mod event_integration_tests {
    use super::*;
    #[test]
    fn lua_chum_tracks_world_clock_and_expires() -> anyhow::Result<()> {
        let collision = sim::Collision::default();
        let mut world = World::new(
            1,
            Player {
                x: 0.5,
                y: 0.5,
                z: 0,
            },
            &collision,
            0,
            0,
        );
        world.clock = Some(sim::GameClock {
            time: 10.0,
            year: 1993,
            month: 6,
            day: 8,
            minutes_per_day: 90.0,
            effective_multiplier: 1.6,
            nights_survived: 0,
        });
        let lua = zomboid_lua::events::EventRuntime::new().map_err(|e| anyhow::anyhow!("{e}"))?;
        lua.install_fishing_save(&zomboid_lua::fishing::FishingSave {
            seed: 1,
            trash_seed: 2,
            chum: vec![],
        })
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        lua.update_fishing_time(
            world
                .clock
                .as_ref()
                .and_then(|c| c.calendar_minutes(&chrono::Local)),
        );
        lua.run_source(
            "FishSchoolManager.getInstance():addChum(100,100,200)",
            "chum",
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        world.clock.as_mut().unwrap().time = 11.0;
        advance_world(&mut world, &collision, (0.0, 0.0), &lua)?;
        assert_eq!(lua.fishing_chum_contribution(100, 100), Some(9));
        world.clock.as_mut().unwrap().time = 15.0;
        advance_world(&mut world, &collision, (0.0, 0.0), &lua)?;
        assert_eq!(lua.fishing_chum_contribution(100, 100), Some(15));
        world.clock.as_mut().unwrap().time = 15.016667;
        advance_world(&mut world, &collision, (0.0, 0.0), &lua)?;
        assert_eq!(lua.fishing_chum_contribution(100, 100), Some(0));
        Ok(())
    }

    #[test]
    fn dispatches_each_step_in_original_order() -> anyhow::Result<()> {
        let collision = sim::Collision::default();
        let mut world = World::new(
            1,
            Player {
                x: 0.5,
                y: 0.5,
                z: 0,
            },
            &collision,
            0,
            0,
        );
        world.clock = Some(sim::GameClock {
            time: 23.99998,
            year: 1993,
            month: 6,
            day: 8,
            minutes_per_day: 90.0,
            effective_multiplier: 1.6,
            nights_survived: 0,
        });
        let lua = zomboid_lua::events::EventRuntime::new().map_err(|e| anyhow::anyhow!("{e}"))?;
        lua.run_source("trace=''; for _, n in ipairs({'EveryDays','EveryHours','EveryTenMinutes','EveryOneMinute'}) do Events[n].Add(function() trace=trace..n..'|' end) end", "test").map_err(|e| anyhow::anyhow!("{e}"))?;
        advance_world(&mut world, &collision, (0.0, 0.0), &lua)?;
        advance_world(&mut world, &collision, (0.0, 0.0), &lua)?;
        let actual = lua
            .eval_string("return trace")
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let line = include_str!("../../../tests/golden/clock_events.txt")
            .lines()
            .find(|l| l.starts_with("EVENT 23.99998@0="))
            .expect("midnight oracle");
        let events = line
            .split_once('=')
            .unwrap()
            .1
            .splitn(3, ',')
            .nth(2)
            .unwrap();
        assert_eq!(actual, format!("{events}|"));
        Ok(())
    }
}

/// First unblocked square on a spiral around (gx, gy) at level 0.
fn spawn(scene: &Scene, gx: f32, gy: f32) -> Player {
    let (cx, cy) = (gx as i32, gy as i32);
    for r in 0..64i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dy.abs()) == r && !scene.collision.blocked(cx + dx, cy + dy, 0) {
                    return Player {
                        x: (cx + dx) as f32 + 0.5,
                        y: (cy + dy) as f32 + 0.5,
                        z: 0,
                    };
                }
            }
        }
    }
    Player { x: gx, y: gy, z: 0 }
}

/// Reads one consistent local-player row through SQLite's read-only connection.
fn load_saved_player(path: &str) -> anyhow::Result<zomboid_content::save::SavedPlayer> {
    let p = std::path::Path::new(path);
    let db = if p.is_dir() {
        p.join("players.db")
    } else {
        p.to_path_buf()
    };
    zomboid_content::save::load_player(&db)
        .map_err(anyhow::Error::msg)?
        .ok_or_else(|| anyhow::anyhow!("no local player in {}", db.display()))
}

#[cfg(test)]
mod save_tests {
    use super::load_saved_player;
    use rusqlite::Connection;

    #[test]
    fn saved_player_includes_committed_wal_without_changing_database() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let db = dir.path().join("players.db");
        let writer = Connection::open(&db)?;
        writer.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
            CREATE TABLE localPlayers(id INTEGER, name TEXT, x REAL, y REAL, z REAL, isDead INTEGER);
            INSERT INTO localPlayers VALUES(1, 'Test', 1.0, 2.0, 0.0, 0);
            PRAGMA wal_checkpoint(TRUNCATE);
            UPDATE localPlayers SET x=42.5, y=67.25, z=1.0 WHERE id=1;")?;
        let original_bytes = std::fs::read(&db)?;
        // Copying just the database loses the committed WAL update: demonstrate
        // the exact client regression with the writer still open.
        let stale = dir.path().join("main-only.db");
        std::fs::copy(&db, &stale)?;
        let old = load_saved_player(stale.to_str().ok_or_else(|| anyhow::anyhow!("path"))?)?;
        assert_eq!((old.x, old.y, old.z), (1.0, 2.0, 0));
        let current =
            load_saved_player(dir.path().to_str().ok_or_else(|| anyhow::anyhow!("path"))?)?;
        assert_eq!((current.x, current.y, current.z), (42.5, 67.25, 1));
        assert_eq!(std::fs::read(&db)?, original_bytes);
        let x: f64 = writer.query_row("SELECT x FROM localPlayers WHERE id=1", [], |row| {
            row.get(0)
        })?;
        assert_eq!(x, 42.5);
        Ok(())
    }
}
