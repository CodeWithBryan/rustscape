//! Persistent static-world buffers over Macroquad's existing miniquad backend.
//! Each Chunk owns its buffers and invalidates them before changing CPU geometry.
//! The same GLSL, atlas, lighting, depth and blend settings are used by both paths.
use macroquad::{miniquad::*, prelude::*};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, Default, serde::Serialize)]
pub struct Statistics {
    pub uploads: u64,
    pub deletes: u64,
    pub resident_meshes: usize,
    pub resident_bytes: usize,
}
pub struct MeshBuffers {
    vertex: BufferId,
    index: BufferId,
    count: i32,
    bytes: usize,
    statistics: Rc<Cell<Statistics>>,
}
impl Drop for MeshBuffers {
    fn drop(&mut self) {
        // Buffers are only created by draw on the render thread. Chunks and
        // their buffers are dropped there while the graphics context is alive.
        let gl = unsafe { get_internal_gl() };
        gl.quad_context.delete_buffer(self.vertex);
        gl.quad_context.delete_buffer(self.index);
        let mut s = self.statistics.get();
        s.deletes += 1;
        s.resident_meshes -= 1;
        s.resident_bytes -= self.bytes;
        self.statistics.set(s);
    }
}

/// A chunk's indices without the triangles roof removal hides this frame. It
/// draws with the chunk's own vertex buffer.
pub struct IndexBuffer {
    id: BufferId,
    count: i32,
}
impl IndexBuffer {
    fn new(ctx: &mut dyn RenderingBackend, indices: &[u16]) -> Self {
        Self {
            id: ctx.new_buffer(
                BufferType::IndexBuffer,
                BufferUsage::Immutable,
                BufferSource::slice(indices),
            ),
            count: indices.len() as i32,
        }
    }
}
impl Drop for IndexBuffer {
    fn drop(&mut self) {
        // Created by draw and dropped on the render thread, like MeshBuffers.
        let gl = unsafe { get_internal_gl() };
        gl.quad_context.delete_buffer(self.id);
    }
}

#[repr(C)]
struct Uniforms {
    projection: [f32; 16],
    model: [f32; 16],
    time: f32,
    shadow_region: [f32; 4],
    shadows: f32,
    light_region: [f32; 4],
    depth_bias: [f32; 2],
    direction: [f32; 3],
    ambient: [f32; 3],
    diffuse0: [f32; 3],
    diffuse1: [f32; 3],
    eye: [f32; 3],
    forward: [f32; 3],
    fog_range: [f32; 2],
    fog_color: [f32; 3],
    material_view: [f32; 4],
}
pub struct View<'a> {
    pub camera: &'a crate::scene_camera::SceneCamera,
    pub atmosphere: rs09_client::atmosphere::Parameters,
    pub direction: [i32; 3],
    pub time: f32,
    pub shadows: bool,
}
pub struct Renderer {
    pipeline: Pipeline,
    /// For blended floor faces: no depth writes.
    blend_pipeline: Pipeline,
    shader: ShaderId,
    statistics: Rc<Cell<Statistics>>,
}
impl Renderer {
    pub fn new() -> Result<Self, String> {
        // Miniquad derives offsets from these formats. Reject a changed Rust
        // vertex layout at compilation, including on WASM.
        const { assert!(std::mem::size_of::<Vertex>() == 40) };
        const { assert!(std::mem::offset_of!(Vertex, uv) == 12) };
        const { assert!(std::mem::offset_of!(Vertex, color) == 20) };
        const { assert!(std::mem::offset_of!(Vertex, normal) == 24) };
        let uniforms = [
            ("Projection", UniformType::Mat4),
            ("Model", UniformType::Mat4),
            ("TextureTime", UniformType::Float1),
            ("ShadowRegion", UniformType::Float4),
            ("ShadowsEnabled", UniformType::Float1),
            ("LightRegion", UniformType::Float4),
            ("DepthBias", UniformType::Float2),
            ("LightDirection", UniformType::Float3),
            ("AmbientLight", UniformType::Float3),
            ("DiffuseLight0", UniformType::Float3),
            ("DiffuseLight1", UniformType::Float3),
            ("CameraPosition", UniformType::Float3),
            ("CameraForward", UniformType::Float3),
            ("FogRange", UniformType::Float2),
            ("FogColor", UniformType::Float3),
            ("MaterialView", UniformType::Float4),
        ];
        let meta = ShaderMeta {
            images: [
                "Texture",
                "MaterialData",
                "ShadowMap",
                "LightGrid",
                "LightData",
                "MaterialTables",
                "MipAtlas",
            ]
            .map(str::to_owned)
            .into(),
            uniforms: UniformBlockLayout {
                uniforms: uniforms
                    .into_iter()
                    .map(|(n, t)| UniformDesc::new(n, t))
                    .collect(),
            },
        };
        let gl = unsafe { get_internal_gl() };
        let ctx = gl.quad_context;
        let shader = ctx
            .new_shader(crate::render::world_shader(), meta)
            .map_err(|e| e.to_string())?;
        let mut pipeline = |depth_write| {
            ctx.new_pipeline(
                &[BufferLayout::default()],
                &[
                    VertexAttribute::new("position", VertexFormat::Float3),
                    VertexAttribute::new("texcoord", VertexFormat::Float2),
                    VertexAttribute::new("color0", VertexFormat::Byte4),
                    VertexAttribute::new("normal", VertexFormat::Float4),
                ],
                shader,
                PipelineParams {
                    cull_face: CullFace::Back,
                    front_face_order: FrontFaceOrder::CounterClockwise,
                    depth_test: Comparison::LessOrEqual,
                    depth_write,
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    )),
                    // The frame stays opaque: a translucent face must not
                    // lower the destination alpha, or a browser canvas (and
                    // a saved capture) shows what is behind the window.
                    alpha_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Zero,
                        BlendFactor::One,
                    )),
                    ..Default::default()
                },
            )
        };
        Ok(Self {
            pipeline: pipeline(true),
            blend_pipeline: pipeline(false),
            shader,
            statistics: Rc::new(Cell::new(Statistics::default())),
        })
    }
    pub fn statistics(&self) -> Statistics {
        self.statistics.get()
    }
    fn upload(&self, ctx: &mut dyn RenderingBackend, mesh: &Mesh) -> MeshBuffers {
        let bytes = std::mem::size_of_val(mesh.vertices.as_slice())
            + std::mem::size_of_val(mesh.indices.as_slice());
        let mut s = self.statistics.get();
        s.uploads += 1;
        s.resident_meshes += 1;
        s.resident_bytes += bytes;
        self.statistics.set(s);
        MeshBuffers {
            vertex: ctx.new_buffer(
                BufferType::VertexBuffer,
                BufferUsage::Immutable,
                BufferSource::slice(&mesh.vertices),
            ),
            index: ctx.new_buffer(
                BufferType::IndexBuffer,
                BufferUsage::Immutable,
                BufferSource::slice(&mesh.indices),
            ),
            count: mesh.indices.len() as i32,
            bytes,
            statistics: self.statistics.clone(),
        }
    }
    pub fn warm(&self, chunk: &mut crate::Chunk) {
        if chunk.gpu.is_none() && !chunk.mesh.indices.is_empty() {
            let gl = unsafe { get_internal_gl() };
            chunk.gpu = Some(self.upload(gl.quad_context, &chunk.mesh));
        }
    }
    /// Draws the chunks `indices`, each depth layer of a chunk with its own
    /// `DepthBias` (`rs09_client::depth_layers`), then the translucent
    /// triangles `glass` in the order given, with depth writes as the
    /// original draws every model face.
    pub fn draw(
        &self,
        world: &mut crate::Workbench,
        indices: &[usize],
        glass: &[crate::roof_render::Glass],
        view: View<'_>,
    ) {
        let [ambient, diffuse0, diffuse1] = view.atmosphere.light_colors();
        let mut uniforms = Uniforms {
            projection: macroquad::camera::Camera::matrix(view.camera).to_cols_array(),
            model: Mat4::IDENTITY.to_cols_array(),
            time: view.time,
            shadow_region: [
                world.shadows.origin.x - 3200.,
                world.shadows.origin.y - 3200.,
                (world.shadows.map.width - 2) as f32 / 16.,
                (world.shadows.map.height - 2) as f32 / 16.,
            ],
            shadows: f32::from(view.shadows),
            light_region: rs09_client::sd_light::light_region([
                world.point_lights.origin.x - 3200.,
                world.point_lights.origin.y - 3200.,
                world.point_lights.size.x,
                world.point_lights.size.y,
            ]),
            depth_bias: [0.; 2],
            direction: [
                view.direction[0] as f32,
                -(view.direction[1] as f32),
                view.direction[2] as f32,
            ],
            ambient,
            diffuse0,
            diffuse1,
            eye: view.camera.position.to_array(),
            forward: (view.camera.target - view.camera.position)
                .normalize()
                .to_array(),
            fog_range: view.atmosphere.fog_range(56 * 128, 512.).map(|n| n / 128.),
            fog_color: rs09_client::atmosphere::rgb(view.atmosphere.fog_color),
            material_view: rs09_client::materials::view(
                (view.camera.target - view.camera.position).to_array(),
            ),
        };
        let textures = [
            world.textures.atlas.raw_miniquad_id(),
            world.textures.metadata.raw_miniquad_id(),
            world.shadows.texture.raw_miniquad_id(),
            world.point_lights.grid.raw_miniquad_id(),
            world.point_lights.data.raw_miniquad_id(),
            world.textures.tables.raw_miniquad_id(),
            world.textures.mips.raw_miniquad_id(),
        ];
        let size = miniquad::window::screen_size();
        // Flush also resets Macroquad's cached state before using raw miniquad.
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let ctx = gl.quad_context;
        ctx.begin_default_pass(PassAction::Nothing);
        // The scene's rectangle (GlRenderer.setViewportBounds), which is the
        // whole window when no interface confines it.
        let (x, y, w, h) = macroquad::camera::Camera::viewport(view.camera).unwrap_or((
            0,
            0,
            size.0 as i32,
            size.1 as i32,
        ));
        ctx.apply_viewport(x, y, w, h);
        ctx.apply_scissor_rect(x, y, w, h);
        ctx.apply_pipeline(&self.pipeline);
        ctx.apply_uniforms(UniformsSource::table(&uniforms));
        let mut blending = false;
        for &index in indices {
            let chunk = &mut world.chunks[index];
            if chunk.blend != blending {
                blending = chunk.blend;
                ctx.apply_pipeline(if blending {
                    &self.blend_pipeline
                } else {
                    &self.pipeline
                });
            }
            let base = crate::roof_render::chunk_bias(chunk);
            let buffers = chunk
                .gpu
                .get_or_insert_with(|| self.upload(ctx, &chunk.mesh));
            match world.roofs.shown(index) {
                Some(shown) if shown.indices.is_empty() => continue,
                Some(shown) => {
                    let buffer = shown
                        .gpu
                        .get_or_insert_with(|| IndexBuffer::new(ctx, &shown.indices));
                    debug_assert!(shown.indices.len() as i32 == buffer.count);
                    ctx.apply_bindings_from_slice(&[buffers.vertex], buffer.id, &textures);
                    for (range, bias) in &shown.groups {
                        uniforms.depth_bias = [base[0] + bias[0], bias[1]];
                        ctx.apply_uniforms(UniformsSource::table(&uniforms));
                        ctx.draw(range.start as i32, range.len() as i32, 1);
                    }
                }
                None => {
                    uniforms.depth_bias = base;
                    ctx.apply_uniforms(UniformsSource::table(&uniforms));
                    ctx.apply_bindings_from_slice(&[buffers.vertex], buffers.index, &textures);
                    ctx.draw(0, buffers.count, 1);
                }
            }
        }
        if blending && !glass.is_empty() {
            ctx.apply_pipeline(&self.pipeline);
        }
        for item in glass {
            let Some(chunk) = world.chunks.get_mut(item.chunk) else {
                continue;
            };
            if item.indices.end > chunk.mesh.indices.len() {
                continue;
            }
            let buffers = chunk
                .gpu
                .get_or_insert_with(|| self.upload(ctx, &chunk.mesh));
            uniforms.depth_bias = item.bias;
            ctx.apply_uniforms(UniformsSource::table(&uniforms));
            ctx.apply_bindings_from_slice(&[buffers.vertex], buffers.index, &textures);
            ctx.draw(item.indices.start as i32, item.indices.len() as i32, 1);
        }
        ctx.end_render_pass();
    }
}
impl Drop for Renderer {
    fn drop(&mut self) {
        let gl = unsafe { get_internal_gl() };
        gl.quad_context.delete_pipeline(self.pipeline);
        gl.quad_context.delete_pipeline(self.blend_pipeline);
        gl.quad_context.delete_shader(self.shader);
    }
}

impl Renderer {
    /// The underwater scene (`underwater_mesh::draw`): each chunk with the
    /// uniforms of its pass, the backdrop through the pipeline without depth
    /// writes. Drawn before any surface chunk of the frame.
    pub fn draw_underwater(
        &self,
        world: &mut crate::Workbench,
        passes: &[(usize, rs09_client::underwater::Uniforms)],
        view: View<'_>,
    ) {
        let forward = (view.camera.target - view.camera.position).normalize();
        let mut uniforms = Uniforms {
            projection: macroquad::camera::Camera::matrix(view.camera).to_cols_array(),
            model: Mat4::IDENTITY.to_cols_array(),
            time: view.time,
            shadow_region: [
                world.shadows.origin.x - 3200.,
                world.shadows.origin.y - 3200.,
                (world.shadows.map.width - 2) as f32 / 16.,
                (world.shadows.map.height - 2) as f32 / 16.,
            ],
            // The shadow map is drawn over the surface only.
            shadows: 0.,
            light_region: [
                world.point_lights.origin.x - 3200.,
                world.point_lights.origin.y - 3200.,
                world.point_lights.size.x,
                world.point_lights.size.y,
            ],
            depth_bias: [0.; 2],
            direction: [
                view.direction[0] as f32,
                -(view.direction[1] as f32),
                view.direction[2] as f32,
            ],
            ambient: [0.; 3],
            diffuse0: [0.; 3],
            diffuse1: [0.; 3],
            eye: view.camera.position.to_array(),
            forward: forward.to_array(),
            fog_range: [0.; 2],
            fog_color: [0.; 3],
            material_view: [0.; 4],
        };
        let textures = [
            world.textures.atlas.raw_miniquad_id(),
            world.textures.metadata.raw_miniquad_id(),
            world.shadows.texture.raw_miniquad_id(),
            world.point_lights.grid.raw_miniquad_id(),
            world.point_lights.data.raw_miniquad_id(),
            world.textures.tables.raw_miniquad_id(),
            world.textures.mips.raw_miniquad_id(),
        ];
        let size = miniquad::window::screen_size();
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let ctx = gl.quad_context;
        ctx.begin_default_pass(PassAction::Nothing);
        let (x, y, w, h) = macroquad::camera::Camera::viewport(view.camera).unwrap_or((
            0,
            0,
            size.0 as i32,
            size.1 as i32,
        ));
        ctx.apply_viewport(x, y, w, h);
        ctx.apply_scissor_rect(x, y, w, h);
        let light_region = uniforms.light_region;
        for (index, pass) in passes {
            // The backdrop's colour is made by its light and is not a lit
            // surface; the floor and the scenery are, with high detail
            // lighting only.
            uniforms.light_region = if pass.depth_write {
                rs09_client::sd_light::light_region(light_region)
            } else {
                light_region
            };
            [uniforms.ambient, uniforms.diffuse0, uniforms.diffuse1] = pass.lights;
            uniforms.fog_range = pass.fog_range;
            uniforms.fog_color = pass.fog_color;
            uniforms.material_view = pass.material_view;
            uniforms.depth_bias = pass.depth_bias;
            ctx.apply_pipeline(if pass.depth_write {
                &self.pipeline
            } else {
                &self.blend_pipeline
            });
            ctx.apply_uniforms(UniformsSource::table(&uniforms));
            let chunk = &mut world.chunks[*index];
            if chunk.mesh.indices.is_empty() {
                continue;
            }
            let buffers = chunk
                .gpu
                .get_or_insert_with(|| self.upload(ctx, &chunk.mesh));
            ctx.apply_bindings_from_slice(&[buffers.vertex], buffers.index, &textures);
            ctx.draw(0, buffers.count, 1);
        }
        ctx.end_render_pass();
    }
}
