use macroquad::prelude::*;
use rs09_client::{model::Model, pack::Pack, surface, texture};
const CELL: usize = 128;
const GRID: usize = 32;
pub struct PointLights {
    pub grid: Texture2D,
    pub data: Texture2D,
    pub lights: Vec<rs09_client::lighting::Light>,
    pixels: Vec<u8>,
    pub corrected: bool,
    pub origin: Vec2,
    pub size: Vec2,
    records: Vec<(rs09_client::lighting::Light, [i32; 2])>,
    window_origin: [i32; 2],
    /// Changes whenever the lights are placed, so what is derived from them
    /// (the terrain light meshes) can be rebuilt.
    pub revision: u64,
    /// `Light.alpha` of each placed light after the last `update`.
    pub alphas: Vec<f32>,
    /// `Light.matchesStaticLightOverride` of each placed light.
    fixed: Vec<bool>,
}
fn next_revision() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
/// `LightingManager.matchesStaticLightOverride`: the reference client's one
/// light that never flickers, by region ID, raw x and y, and level.
fn static_light_override(raw: &rs09_client::lighting::Light, region: [i32; 2]) -> bool {
    ((region[0] >> 6) << 8) + (region[1] >> 6) == 8005
        && raw.coordinates[0] == 3063
        && raw.coordinates[1] == 3046
        && raw.level == 2
}
struct PlacedLights {
    lights: Vec<rs09_client::lighting::Light>,
    pixels: Vec<u8>,
    grid: Vec<u8>,
    fixed: Vec<bool>,
}
pub struct PreparedLights {
    origin: Vec2,
    size: Vec2,
    records: Vec<(rs09_client::lighting::Light, [i32; 2])>,
    window_origin: [i32; 2],
    corrected: bool,
    placed: PlacedLights,
}
impl PreparedLights {
    pub fn new(
        regions: &[rs09_client::terrain::Region],
        window_origin: [i32; 2],
        corrected: bool,
        mut flicker: impl FnMut(i32) -> rs09_protocol::Result<rs09_client::lighting::Flicker>,
    ) -> Result<Self, String> {
        let origin = vec2(
            regions.iter().map(|r| r.x * 64).min().unwrap() as f32,
            regions.iter().map(|r| r.z * 64).min().unwrap() as f32,
        );
        let size = vec2(
            regions.iter().map(|r| r.x * 64 + 64).max().unwrap() as f32,
            regions.iter().map(|r| r.z * 64 + 64).max().unwrap() as f32,
        ) - origin;
        let palette = surface::palette(0.7);
        let mut records = Vec::new();
        for region in regions {
            for bytes in &region.point_lights {
                let light = rs09_client::lighting::Light::decode(bytes, &palette, &mut flicker)
                    .map_err(|e| e.to_string())?;
                records.push((light, [region.x * 64, region.z * 64]));
            }
        }
        let placed = place_lights(regions, &records, origin, size, window_origin, corrected);
        Ok(Self {
            origin,
            size,
            records,
            window_origin,
            corrected,
            placed,
        })
    }
}
fn place_lights(
    regions: &[rs09_client::terrain::Region],
    records: &[(rs09_client::lighting::Light, [i32; 2])],
    origin: Vec2,
    size: Vec2,
    window_origin: [i32; 2],
    corrected: bool,
) -> PlacedLights {
    let mut lights = Vec::new();
    let mut fixed = Vec::new();
    let mut pixels = vec![0; 4096];
    for (raw, region) in records {
        // LightingManager.addLight retains the first 255 accepted records.
        if lights.len() >= 255 {
            break;
        }
        let mut light = raw.clone();
        let world = [
            raw.coordinates[0] + region[0] * 128,
            raw.coordinates[1] + region[1] * 128,
        ];
        let index = lights.len();
        let Some(region_tiles) = regions
            .iter()
            .find(|r| r.x == (world[0] >> 13) && r.z == (world[1] >> 13))
        else {
            continue;
        };
        let ground = region_tiles
            .tile(
                light.level,
                (world[0] >> 7 & 63) as usize,
                (world[1] >> 7 & 63) as usize,
            )
            .height;
        let (horizontal, vertical) = if corrected {
            (world, ground - raw.coordinates[2])
        } else {
            let Some(p) = rs09_client::lighting::pinned_position(
                raw.coordinates,
                *region,
                window_origin,
                ground,
            ) else {
                continue;
            };
            (
                [window_origin[0] * 128 + p[0], window_origin[1] * 128 + p[1]],
                p[2],
            )
        };
        light.coordinates = [
            horizontal[0] - (origin.x * 128.0) as i32,
            horizontal[1] - (origin.y * 128.0) as i32,
            vertical,
        ];
        let x = light.coordinates[0];
        let z = light.coordinates[1];
        let height = -vertical;
        let base = index * 4;
        pixels[base..base + 2].copy_from_slice(&(x as u16).to_be_bytes());
        pixels[base + 2..base + 4].copy_from_slice(&(z as u16).to_be_bytes());
        pixels[1024 + base..1026 + base].copy_from_slice(&((height + 32768) as u16).to_be_bytes());
        pixels[1026 + base..1028 + base]
            .copy_from_slice(&(((light.radius << 7) + 64) as u16).to_be_bytes());
        pixels[2048 + base..2051 + base].copy_from_slice(&[
            (light.color >> 16) as u8,
            (light.color >> 8) as u8,
            light.color as u8,
        ]);
        lights.push(light);
        fixed.push(static_light_override(raw, *region));
    }

    let grid = rs09_client::lighting::grid(size.x as usize, size.y as usize, &lights)
        .into_iter()
        .flatten()
        .collect();
    PlacedLights {
        lights,
        pixels,
        grid,
        fixed,
    }
}
impl PointLights {
    pub fn new(
        regions: &[rs09_client::terrain::Region],
        pack: &Pack,
        window_origin: [i32; 2],
    ) -> Result<Self, String> {
        let prepared = PreparedLights::new(regions, window_origin, false, |id| {
            rs09_client::lighting::Flicker::decode(
                pack.get(&format!("light/{id}"))
                    .map_err(|_| rs09_protocol::Error::new(0, "missing custom light type"))?,
            )
        })?;
        Ok(Self::from_prepared(prepared))
    }
    pub fn from_prepared(prepared: PreparedLights) -> Self {
        Self::with_textures(prepared, None)
    }
    pub fn with_textures(prepared: PreparedLights, spare: Option<[Texture2D; 2]>) -> Self {
        let placed = prepared.placed;
        let dims = [prepared.size.x as u32, (prepared.size.y * 4.0) as u32];
        let [grid, data] = match spare {
            Some([grid, data])
                if grid.width() as u32 == dims[0] && grid.height() as u32 == dims[1] =>
            {
                grid.update_from_bytes(dims[0], dims[1], &placed.grid);
                data.update_from_bytes(256, 4, &placed.pixels);
                [grid, data]
            }
            _ => {
                let grid = Texture2D::from_rgba8(dims[0] as u16, dims[1] as u16, &placed.grid);
                grid.set_filter(FilterMode::Nearest);
                let data = Texture2D::from_rgba8(256, 4, &placed.pixels);
                data.set_filter(FilterMode::Nearest);
                [grid, data]
            }
        };
        Self {
            grid,
            data,
            lights: placed.lights,
            pixels: placed.pixels,
            corrected: prepared.corrected,
            origin: prepared.origin,
            size: prepared.size,
            records: prepared.records,
            window_origin: prepared.window_origin,
            revision: next_revision(),
            alphas: Vec::new(),
            fixed: placed.fixed,
        }
    }
    pub fn spare_textures() -> [Texture2D; 2] {
        let grid = Texture2D::from_rgba8(192, 192 * 4, &vec![0; 192 * 192 * 16]);
        grid.set_filter(FilterMode::Nearest);
        let data = Texture2D::from_rgba8(256, 4, &[0; 4096]);
        data.set_filter(FilterMode::Nearest);
        [grid, data]
    }
    pub fn into_textures(self) -> [Texture2D; 2] {
        [self.grid, self.data]
    }
    pub fn place(&mut self, regions: &[rs09_client::terrain::Region], corrected: bool) {
        self.corrected = corrected;
        let placed = place_lights(
            regions,
            &self.records,
            self.origin,
            self.size,
            self.window_origin,
            corrected,
        );
        self.lights = placed.lights;
        self.pixels = placed.pixels;
        self.fixed = placed.fixed;
        self.revision = next_revision();
        self.grid
            .update_from_bytes(self.size.x as u32, self.size.y as u32 * 4, &placed.grid);
    }
    /// `LightingManager.updateAllLightAnimations(client.loop,
    /// !Preferences.flickeringEffectsOn)`. The reference client's one fixed
    /// light (`matchesStaticLightOverride`) never flickers.
    pub fn update(&mut self, tick: i32, disable_flicker: bool) {
        self.alphas.resize(self.lights.len(), 0.0);
        for (index, light) in self.lights.iter().enumerate() {
            let still = disable_flicker || self.fixed.get(index).copied().unwrap_or(false);
            let alpha = light
                .sample(tick, still, rs09_client::lighting::noise())
                .alpha;
            self.alphas[index] = alpha;
            self.pixels[3072 + index * 4..3074 + index * 4]
                .copy_from_slice(&(((alpha * 2048.0) as i32 + 32768) as u16).to_be_bytes());
        }
        self.data.update_from_bytes(256, 4, &self.pixels);
    }
}
pub struct Shadows {
    pub map: rs09_client::shadow::Map,
    pub texture: Texture2D,
    pub origin: Vec2,
    pub light: [i32; 2],
    /// The scenery-shadows preference: 0 hides the map, 2 also rebuilds the
    /// shadows of animated scenery as they move.
    pub mode: i32,
}
pub struct ShadowMap {
    pub map: rs09_client::shadow::Map,
    pub origin: Vec2,
    pub light: [i32; 2],
}
impl ShadowMap {
    pub fn new(origin: Vec2, columns: usize, rows: usize, direction: [i32; 3]) -> Self {
        Self {
            map: rs09_client::shadow::Map::new(columns, rows),
            origin,
            light: rs09_client::shadow::light_projection(direction),
        }
    }
    pub fn position(&self, position: Vec2, elevation: i32) -> [i32; 2] {
        shadow_position(self.origin, self.light, position, elevation)
    }
}
fn shadow_position(origin: Vec2, light: [i32; 2], position: Vec2, elevation: i32) -> [i32; 2] {
    let fine = (position - origin) * 128.0;
    let fine = [fine.x as i32, fine.y as i32];
    std::array::from_fn(|i| fine[i].wrapping_sub(elevation.wrapping_mul(light[i]) >> 8) >> 3)
}
impl Shadows {
    pub fn from_map(source: ShadowMap) -> Self {
        Self::with_texture(source, None)
    }
    pub fn with_texture(source: ShadowMap, spare: Option<Texture2D>) -> Self {
        let width = source.map.width - 2;
        let height = source.map.height - 2;
        let texture = spare
            .filter(|tex| tex.width() as usize == width && tex.height() as usize == height)
            .unwrap_or_else(|| {
                Texture2D::from_rgba8(width as u16, height as u16, &vec![0; width * height * 4])
            });
        texture.set_filter(FilterMode::Linear);
        Self {
            map: source.map,
            texture,
            origin: source.origin,
            light: source.light,
            mode: 2,
        }
    }
    pub fn position(&self, position: Vec2, elevation: i32) -> [i32; 2] {
        shadow_position(self.origin, self.light, position, elevation)
    }
    pub fn upload_tile(&self, [x, y]: [usize; 2], bytes: Vec<u8>) {
        self.texture.update_part(
            &Image {
                width: 128,
                height: 128,
                bytes,
            },
            x as i32,
            y as i32,
            128,
            128,
        );
    }
    pub fn upload(&mut self) {
        for ([x, y], bytes) in self.map.updates() {
            self.upload_tile([x, y], bytes);
        }
    }
}
pub struct Textures {
    pub atlas: Texture2D,
    pub metadata: Texture2D,
    /// `rs09_client::materials::tables`: noise volumes, liquid table, material types.
    pub tables: Texture2D,
    /// Mipmap levels 1 to 3 of every texture, one 128 by 64 cell each.
    pub mips: Texture2D,
    pub generated: usize,
    pub materials: Vec<Option<texture::Material>>,
    palette: Vec<u32>,
    /// The `floorshadows` sprites, when the pack has them.
    pub floor_shadows: Option<std::sync::Arc<rs09_client::shadow_rules::FloorSprites>>,
}
impl Textures {
    pub fn worker_copy(&self) -> Self {
        Self {
            atlas: self.atlas.weak_clone(),
            metadata: self.metadata.weak_clone(),
            tables: self.tables.weak_clone(),
            mips: self.mips.weak_clone(),
            generated: self.generated,
            materials: self.materials.clone(),
            palette: self.palette.clone(),
            floor_shadows: self.floor_shadows.clone(),
        }
    }
    pub fn load(pack: &Pack) -> Result<Self, String> {
        let images = TextureImages::generate(pack)?;
        let side = (CELL * GRID) as u16;
        let atlas = Texture2D::from_rgba8(side, side, &images.atlas);
        atlas.set_filter(FilterMode::Nearest);
        let metadata = Texture2D::from_rgba8((GRID * GRID) as u16, 2, &images.metadata);
        metadata.set_filter(FilterMode::Nearest);
        let [width, height] = rs09_client::materials::TABLES;
        let tables = Texture2D::from_rgba8(width as u16, height as u16, &images.tables);
        tables.set_filter(FilterMode::Nearest);
        let [width, height] = images.mips_size;
        let mips = Texture2D::from_rgba8(width as u16, height as u16, &images.mips);
        mips.set_filter(FilterMode::Nearest);
        Ok(Self {
            atlas,
            metadata,
            tables,
            mips,
            generated: images.generated,
            materials: images.materials,
            palette: surface::palette(0.7),
            // By name from the cache, or by the ID a prepared pack lists.
            floor_shadows: pack
                .get("sprite/floorshadows")
                .ok()
                .or_else(|| {
                    let ids: serde_json::Value =
                        serde_json::from_slice(pack.get("minimap/assets").ok()?).ok()?;
                    pack.get(&format!("sprite/{}", ids["floorshadows"].as_u64()?))
                        .ok()
                })
                .and_then(|data| rs09_client::shadow_rules::FloorSprites::decode(data).ok())
                .map(std::sync::Arc::new),
        })
    }
    /// `Js5GlTextureProvider.setLowDetail` (CS2 6006) clears the texture
    /// cache, and every texture is generated again at its new size when it is
    /// next bound. Here all of them are generated at once, into the textures
    /// the scene's meshes and materials already hold.
    pub fn regenerate(&mut self, pack: &Pack) -> Result<(), String> {
        let images = TextureImages::generate(pack)?;
        let side = (CELL * GRID) as u32;
        self.atlas.update_from_bytes(side, side, &images.atlas);
        self.metadata
            .update_from_bytes((GRID * GRID) as u32, 2, &images.metadata);
        let [width, height] = rs09_client::materials::TABLES;
        self.tables
            .update_from_bytes(width as u32, height as u32, &images.tables);
        let [width, height] = images.mips_size;
        self.mips
            .update_from_bytes(width as u32, height as u32, &images.mips);
        self.generated = images.generated;
        Ok(())
    }
    pub fn mesh(&self) -> Mesh {
        Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: Some(self.atlas.clone()),
        }
    }
}
/// The pixels of the four material textures, before they are uploaded.
struct TextureImages {
    atlas: Vec<u8>,
    metadata: Vec<u8>,
    tables: Vec<u8>,
    mips: Vec<u8>,
    mips_size: [usize; 2],
    generated: usize,
    materials: Vec<Option<texture::Material>>,
}
impl TextureImages {
    fn generate(pack: &Pack) -> Result<Self, String> {
        let materials = texture::materials(pack.get("materials")?).map_err(|e| e.to_string())?;
        if materials.len() >= GRID * GRID {
            return Err("material atlas capacity exceeded".into());
        }
        let side = CELL * GRID;
        let mut rgba = vec![255; side * side * 4];
        let mut metadata = vec![0; GRID * GRID * 2 * 4];
        let mut generated = 0;
        let mut mips = rs09_client::materials::MipAtlas::new(GRID);
        let mut mipmaps = vec![0; materials.len()];
        for (id, material) in materials.iter().enumerate() {
            let Some(material) = material else {
                continue;
            };
            // Js5GlTextureProvider.isLowDetail: the texture's own flag, or
            // the high-detail textures preference turned off.
            let size = if texture::low_detail(material) {
                64
            } else {
                128
            };
            let graph = texture::Graph::decode(pack.get(&format!("texture/{id}"))?)
                .map_err(|e| format!("Material {id}: {e}"))?;
            metadata[id * 4] = u8::from(graph.wrap[0]) * 255;
            metadata[id * 4 + 1] = u8::from(graph.wrap[1]) * 255;
            metadata[id * 4 + 2] = graph.blend_mode * 85;
            metadata[(GRID * GRID + id) * 4] = (i16::from(graph.scroll[0]) + 128) as u8;
            metadata[(GRID * GRID + id) * 4 + 1] = (i16::from(graph.scroll[1]) + 128) as u8;
            metadata[(GRID * GRID + id) * 4 + 2] =
                u8::from(texture::primary_alpha(Some(material))) * 255;
            metadata[id * 4 + 3] = if size == 64 { 128 } else { 255 };
            let pixels = texture::generate(pack, id as i32, size)
                .map_err(|e| format!("Material {id}: {e}"))?;
            generated += 1;
            mipmaps[id] = graph.mipmap;
            mips.add(id, &pixels, size, graph.mipmap, |side| {
                texture::generate(pack, id as i32, side)
            })
            .map_err(|e| format!("Material {id} mipmaps: {e}"))?;
            for y in 0..size {
                let start = ((id / GRID * CELL + y) * side + (id % GRID) * CELL) * 4;
                rgba[start..start + size * 4]
                    .copy_from_slice(&pixels[y * size * 4..(y + 1) * size * 4]);
            }
        }
        metadata[(GRID * GRID - 1) * 4 + 3] = 255;
        Ok(Self {
            atlas: rgba,
            metadata,
            tables: rs09_client::materials::tables(&materials, &mipmaps),
            mips_size: mips.size(),
            mips: mips.rgba,
            generated,
            materials,
        })
    }
}
fn vertex(mesh: &mut Mesh, p: Vec3, uv: Vec2, color: [u8; 4], normal: Vec3, id: i32) {
    mesh.vertices.push(Vertex {
        position: p,
        uv,
        color,
        normal: vec4(
            normal.x,
            normal.y,
            normal.z,
            if id < 0 {
                (GRID * GRID - 1) as f32
            } else {
                id as f32
            },
        ),
    });
}
pub fn model(
    mesh: &mut Mesh,
    m: &Model,
    textures: &Textures,
    mut position: impl FnMut([i32; 3]) -> Vec3,
    mut direction: impl FnMut([i32; 4]) -> Vec3,
) {
    let computed = if m.surfaces.is_empty() {
        surface::prepare(m, &textures.materials)
    } else {
        Vec::new()
    };
    let surfaces = if m.surfaces.is_empty() {
        &computed
    } else {
        &m.surfaces
    };
    let order;
    let order = if m.draw_order.is_empty() {
        order = surface::face_order(m, &textures.materials);
        &order
    } else {
        &m.draw_order
    };
    let baked = rs09_client::sd_light::bake(
        if m.lighting != [0; 2] {
            m.lighting[0]
        } else {
            64
        },
        if m.lighting[1] > 0 {
            m.lighting[1]
        } else {
            768
        },
    );
    for &i in order {
        let face = m.faces[i];
        if !surface::visible(m, i, &textures.materials) {
            continue;
        }
        let tex = m.textures[i];
        let mat = if tex >= 0 {
            textures
                .materials
                .get(tex as usize)
                .and_then(Option::as_ref)
        } else {
            None
        };
        let color = surface::pack_color(
            m.colors[i],
            mat,
            if m.lighting != [0; 2] {
                m.lighting[0]
            } else {
                64
            },
            m.alpha[i],
            &textures.palette,
        );
        let start = mesh.vertices.len() as u16;
        for (j, v) in face.into_iter().enumerate() {
            let point = position(m.vertices[v]);
            let n = surfaces[i].normals[j];
            let contrast = if m.lighting[1] > 0 {
                m.lighting[1]
            } else {
                768
            };
            // Without high detail lighting each packed vertex has its own
            // baked lightness (GlModel.packInterleavedVertexData).
            let color = baked.as_ref().map_or(color, |bake| {
                surface::pack_color(
                    m.colors[i],
                    mat,
                    bake.light.lightness(n),
                    m.alpha[i],
                    &bake.palette,
                )
            });
            let scale = if n[3] == 0 {
                3.0 / (contrast + contrast / 2) as f32
            } else {
                (3.0 / contrast as f32) / n[3] as f32
            };
            let normal = direction(n) * scale;
            vertex(
                mesh,
                point,
                Vec2::from_array(rs09_client::texcoord::gpu(surfaces[i].uv[j])),
                color,
                normal,
                tex,
            );
        }
        mesh.indices.extend([start, start + 1, start + 2]);
    }
}
/// Shared GLSL for streaming meshes and persistent static buffers: the
/// template below with `rs09_client::materials`' code at its marker lines.
pub fn world_shader() -> miniquad::ShaderSource<'static> {
    let ShaderSource::Glsl { vertex, fragment } = world_shader_template() else {
        unreachable!("the world shader is GLSL")
    };
    // The depth layer of each vertex: `rs09_client::depth_layers`.
    let vertex = vertex.replace("//@depth-layers\n", &rs09_client::depth_layers::glsl());
    let [vertex, fragment] = rs09_client::materials::world_shader(&vertex, fragment);
    ShaderSource::Glsl { vertex, fragment }
}
fn world_shader_template() -> miniquad::ShaderSource<'static> {
    ShaderSource::Glsl {
        vertex: r#"#version 100
attribute vec3 position;attribute vec2 texcoord;attribute vec4 color0;attribute vec4 normal;
varying highp vec2 uv;varying lowp vec4 color;varying highp float materialId;varying highp float fogDistance;varying highp vec2 shadowUv;
varying lowp vec3 floorLight;
uniform mat4 Model;uniform mat4 Projection;
uniform vec3 LightDirection;uniform vec3 AmbientLight;uniform vec3 DiffuseLight0;uniform vec3 DiffuseLight1;
uniform vec3 CameraPosition;uniform vec3 CameraForward;
uniform vec4 ShadowRegion;
uniform vec2 DepthBias;
uniform sampler2D LightGrid;uniform sampler2D LightData;uniform vec4 LightRegion;
float pair(vec2 v){return dot(floor(v*255.0+vec2(0.5)),vec2(256.0,1.0));}
vec3 localLight(float encoded,vec3 point,vec3 n,bool floorSurface){
 if(encoded<0.5)return vec3(0.0);float id=encoded-1.0;float u=(id+0.5)/256.0;
 vec4 xz=texture2D(LightData,vec2(u,0.125));vec4 yr=texture2D(LightData,vec2(u,0.375));
 vec3 tint=texture2D(LightData,vec2(u,0.625)).rgb;
 float alpha=(pair(texture2D(LightData,vec2(u,0.875)).rg)-32768.0)/2048.0;
 vec3 p=vec3(pair(xz.rg)/128.0+LightRegion.x,(pair(yr.rg)-32768.0)/128.0,pair(xz.ba)/128.0+LightRegion.y);
 vec3 delta=p-point;float d2=dot(delta,delta);float radius=pair(yr.ba)/128.0;
 if(d2<=0.0)return vec3(0.0);float lambert=max(dot(n,delta/sqrt(d2)),0.0);
 if(floorSurface){float intensity=min(lambert*max(1.0-d2/(radius*radius),0.0)*2.0,1.0);return floor(intensity*tint*255.0)/255.0*alpha;}
 return tint*alpha*lambert*(radius*radius/d2);
}
// The four lights of one grid tile; none outside the grid.
vec4 cell(vec2 t,float plane){
 if(t.x<0.0||t.y<0.0||t.x>=LightRegion.z||t.y>=LightRegion.w||plane>3.5)return vec4(0.0);
 return floor(texture2D(LightGrid,vec2((t.x+0.5)/LightRegion.z,(t.y+plane*LightRegion.w+0.5)/(LightRegion.w*4.0)))*255.0+vec4(0.5));
}
void put(inout vec4 a,inout float n,float v){
 if(v<0.5||n>3.5)return;
 if(abs(a.r-v)<0.5||abs(a.g-v)<0.5||abs(a.b-v)<0.5||abs(a.a-v)<0.5)return;
 if(n<0.5)a.r=v;else if(n<1.5)a.g=v;else if(n<2.5)a.b=v;else a.a=v;
 n+=1.0;
}
float keep(float v,vec4 a,vec4 b){
 if(v<0.5)return 0.0;
 if(abs(a.r-v)<0.5||abs(a.g-v)<0.5||abs(a.b-v)<0.5||abs(a.a-v)<0.5)return v;
 if(abs(b.r-v)<0.5||abs(b.g-v)<0.5||abs(b.b-v)<0.5||abs(b.a-v)<0.5)return v;
 return 0.0;
}
// rs09_client::light_select::resolve: the lights LightingManager leaves
// active for the object this vertex belongs to, from its baked key.
vec4 objectLights(float key,float plane){
 vec2 vt=floor(position.xz-LightRegion.xy);
 if(key<0.5)return cell(vt,plane);
 vec4 ids=vec4(0.0);
 if(key>1023.5){
  float k=key-1024.0;float n=0.0;
  vec2 o=vt+vec2(floor(k/128.0),mod(floor(k/16.0),8.0))-vec2(6.0);
  float w=mod(floor(k/4.0),4.0)+1.0;float l=mod(k,4.0)+1.0;
  for(int i=0;i<4;i++){for(int j=0;j<4;j++){
   if(float(i)<w&&float(j)<l){vec4 c=cell(o+vec2(float(i),float(j)),plane);put(ids,n,c.r);put(ids,n,c.g);put(ids,n,c.b);put(ids,n,c.a);}
  }}
  return ids;
 }
 vec2 o=vt+vec2(floor(key/256.0),mod(floor(key/64.0),4.0))-vec2(2.0);
 float code=mod(floor(key/4.0),16.0);float raised=mod(floor(key/2.0),2.0);
 vec4 own=cell(o,plane);
 vec2 cam=CameraPosition.xz-LightRegion.xy;
 vec2 side=sign(floor(cam)-o);
 vec2 f=o*128.0+vec2(64.0)-floor(cam*128.0);
 if(code>11.5){
  vec2 a;vec2 b;
  if(mod(code,2.0)>0.5){
   if(f.y>-f.x){a=o+vec2(0.0,-1.0);b=o+vec2(-1.0,0.0);}else{a=o+vec2(0.0,1.0);b=o+vec2(1.0,0.0);}
  }else if(f.y>f.x){a=o+vec2(0.0,-1.0);b=o+vec2(1.0,0.0);}else{a=o+vec2(0.0,1.0);b=o+vec2(-1.0,0.0);}
  vec4 ca=cell(a,plane);vec4 cb=cell(b,plane);
  return vec4(keep(own.r,ca,cb),keep(own.g,ca,cb),keep(own.b,ca,cb),keep(own.a,ca,cb));
 }
 float fi=code<7.5?code:code-4.0;vec2 d;
 if(fi<0.5)d=vec2(-1.0,0.0);else if(fi<1.5)d=vec2(0.0,1.0);else if(fi<2.5)d=vec2(1.0,0.0);else if(fi<3.5)d=vec2(0.0,-1.0);
 else if(fi<4.5)d=vec2(-1.0,1.0);else if(fi<5.5)d=vec2(1.0,1.0);else if(fi<6.5)d=vec2(1.0,-1.0);else d=vec2(-1.0,-1.0);
 bool late;
 if(fi>3.5)late=side.x==d.x&&side.y==d.y;else if(d.x!=0.0)late=side.x==d.x;else late=side.y==d.y;
 vec2 nb=o+d;bool inside=nb.x>=0.0&&nb.y>=0.0&&nb.x<LightRegion.z&&nb.y<LightRegion.w;
 if(late){if(inside)return cell(nb,plane+raised);return own;}
 if(code>7.5){
  float dotp=d.x*side.x+d.y*side.y;float span=abs(side.x)+abs(side.y);
  bool deferred=(span>1.5&&dotp==0.0)||(span>0.5&&span<1.5&&dotp==1.0);
  float k=code-8.0;bool beyond;
  if(k<0.5)beyond=f.y+64.0<f.x-64.0;else if(k<1.5)beyond=f.y+64.0<-(f.x+64.0);else if(k<2.5)beyond=f.y-64.0>f.x+64.0;else beyond=f.y-64.0>-(f.x-64.0);
  if(deferred&&beyond&&inside)return cell(nb,plane);
 }
 return own;
}
//@depth-layers
//@material-vertex
void main(){
 float lightKey=floor(normal.w/8192.0);float surface=normal.w-lightKey*8192.0;
 gl_Position=Projection*Model*vec4(position,1.0);uv=texcoord;materialId=surface;
 gl_Position.z+=depthLayer(surface);
 float d=dot(normal.xyz,normalize(LightDirection));
 vec3 light=AmbientLight+DiffuseLight0*max(d,0.0)+DiffuseLight1*max(-d,0.0);
 float plane=floor(surface/2048.0);bool floorSurface=mod(surface,2048.0)>=1024.0;
 // Terrain is lit by the light meshes drawn over it, not here.
 floorLight=vec3(0.0);
 if(!floorSurface){
  vec4 ids=objectLights(lightKey,plane);
  light+=localLight(ids.r,position,normal.xyz,false)+localLight(ids.g,position,normal.xyz,false)+localLight(ids.b,position,normal.xyz,false)+localLight(ids.a,position,normal.xyz,false);
 }
 // rs09_client::sd_light::light_region: GL lighting is off without high
 // detail lighting, and the vertex colours are the baked ones.
 if(LightRegion.z<0.0)light=vec3(1.0);
 color=vec4(clamp(color0.rgb/255.0*light,0.0,1.0),color0.a/255.0);
 fogDistance=abs(dot(position-CameraPosition,CameraForward));
 shadowUv=(position.xz-ShadowRegion.xy)/ShadowRegion.zw;
 materialVertex();
}
"#,
        fragment: r#"#version 100
//@material-header
precision highp float;
varying highp vec2 uv;varying lowp vec4 color;varying highp float materialId;varying highp float fogDistance;varying highp vec2 shadowUv;
varying lowp vec3 floorLight;
uniform sampler2D Texture;uniform sampler2D MaterialData;uniform float TextureTime;
uniform sampler2D ShadowMap;uniform float ShadowsEnabled;
uniform vec4 ShadowRegion;
uniform vec3 FogColor;uniform vec2 FogRange;
vec4 sampleTex(vec2 pixel,vec2 origin,vec2 wrap,float size){
 vec2 repeated=mod(mod(pixel,vec2(size))+vec2(size),vec2(size));
 vec2 bounded=clamp(pixel,vec2(0.0),vec2(size-1.0));
 vec2 coord=mix(bounded,repeated,wrap);
 return texture2D(Texture,(origin+coord+vec2(0.5))/4096.0);
}
//@material-fragment
void main(){
 float id=mod(floor(materialId+0.5),1024.0);vec4 info=texture2D(MaterialData,vec2((id+0.5)/1024.0,0.25));
 vec4 animationData=texture2D(MaterialData,vec2((id+0.5)/1024.0,0.75));
 vec2 scroll=animationData.rg*255.0-vec2(128.0);
 scroll*=materialScroll(id);
 float size=info.a<0.75?64.0:128.0;vec2 origin=vec2(mod(id,32.0),floor(id/32.0))*128.0;
 vec2 point=uv*size+scroll*TextureTime-vec2(0.5);vec2 base=floor(point);vec2 f=fract(point);
 vec4 a=sampleTex(base,origin,info.rg,size);vec4 b=sampleTex(base+vec2(1.0,0.0),origin,info.rg,size);
 vec4 c=sampleTex(base+vec2(0.0,1.0),origin,info.rg,size);vec4 d=sampleTex(base+vec2(1.0,1.0),origin,info.rg,size);
 vec4 tex=mix(mix(a,b,f.x),mix(c,d,f.x),f.y);tex=materialMip(tex,point,origin,info.rg,size);vec4 result=color*tex;
 if(animationData.b>0.5)result.a=color.a;
 float mode=floor(info.b*3.0+0.5);
 if(mode==1.0)result.rgb=clamp(tex.rgb+color.rgb,0.0,1.0);
 if(mode==2.0)result.rgb=clamp(tex.rgb-color.rgb,0.0,1.0);
 if(mode==3.0)result.rgb=clamp(tex.rgb+color.rgb-vec3(0.5),0.0,1.0);
 result=materialSurface(result,tex);
 vec2 shadowSize=ShadowRegion.zw*16.0;vec2 sp=shadowUv*shadowSize;
 vec2 shadowCoord=floor(sp/128.0)*128.0+clamp(mod(sp,128.0),vec2(0.5),vec2(127.5));
 if(mod(materialId,2048.0)>=1024.0&&materialId<2048.0)result.rgb*=1.0-texture2D(ShadowMap,shadowCoord/shadowSize).a*ShadowsEnabled;
 float fogFactor=clamp((FogRange.y-fogDistance)/(FogRange.y-FogRange.x),0.0,1.0);
 if(result.a<=0.0)discard;result.rgb=mix(FogColor,result.rgb,fogFactor);result.rgb*=vec3(1.0)+floorLight*fogFactor;gl_FragColor=result;
 gl_FragColor=materialOutput(result);
}
"#,
    }
}
pub fn material(textures: &Textures, depth_write: bool) -> Result<Material, macroquad::Error> {
    use macroquad::miniquad::{
        BlendFactor, BlendState, BlendValue, Comparison, CullFace, Equation, FrontFaceOrder,
        PipelineParams,
    };
    let material = load_material(
        world_shader(),
        MaterialParams {
            pipeline_params: PipelineParams {
                cull_face: CullFace::Back,
                front_face_order: FrontFaceOrder::CounterClockwise,
                depth_test: Comparison::LessOrEqual,
                depth_write,
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                // As `static_render`'s pipeline: the frame stays opaque
                // (see `rs09_client::frame_dump`).
                alpha_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Zero,
                    BlendFactor::One,
                )),
                ..Default::default()
            },
            textures: vec![
                "MaterialData".into(),
                "ShadowMap".into(),
                "LightGrid".into(),
                "LightData".into(),
                "MaterialTables".into(),
                "MipAtlas".into(),
            ],
            uniforms: vec![
                UniformDesc::new("TextureTime", UniformType::Float1),
                UniformDesc::new("ShadowRegion", UniformType::Float4),
                UniformDesc::new("ShadowsEnabled", UniformType::Float1),
                UniformDesc::new("LightRegion", UniformType::Float4),
                UniformDesc::new("DepthBias", UniformType::Float2),
                UniformDesc::new("LightDirection", UniformType::Float3),
                UniformDesc::new("AmbientLight", UniformType::Float3),
                UniformDesc::new("DiffuseLight0", UniformType::Float3),
                UniformDesc::new("DiffuseLight1", UniformType::Float3),
                UniformDesc::new("CameraPosition", UniformType::Float3),
                UniformDesc::new("CameraForward", UniformType::Float3),
                UniformDesc::new("FogRange", UniformType::Float2),
                UniformDesc::new("FogColor", UniformType::Float3),
                UniformDesc::new("MaterialView", UniformType::Float4),
            ],
        },
    )?;
    material.set_texture("MaterialData", textures.metadata.clone());
    material.set_texture("MaterialTables", textures.tables.clone());
    material.set_texture("MipAtlas", textures.mips.clone());
    material.set_uniform("TextureTime", 0.0f32);
    // No depth layer (`rs09_client::depth_layers`) until a scene draw asks.
    material.set_uniform("DepthBias", vec2(0.0, 1.0));
    Ok(material)
}

pub fn atmosphere(
    material: &Material,
    params: rs09_client::atmosphere::Parameters,
    direction: [i32; 3],
    eye: Vec3,
    target: Vec3,
) {
    let [ambient, diffuse0, diffuse1] = params.light_colors();
    material.set_uniform("AmbientLight", Vec3::from_array(ambient));
    material.set_uniform("DiffuseLight0", Vec3::from_array(diffuse0));
    material.set_uniform("DiffuseLight1", Vec3::from_array(diffuse1));
    material.set_uniform(
        "LightDirection",
        vec3(
            direction[0] as f32,
            -(direction[1] as f32),
            direction[2] as f32,
        ),
    );
    material.set_uniform("CameraPosition", eye);
    material.set_uniform("CameraForward", (target - eye).normalize());
    material.set_uniform(
        "MaterialView",
        Vec4::from_array(rs09_client::materials::view((target - eye).to_array())),
    );
    // Pinned GlobalConfig defaults: 56 tiles, four tiles of fade distance.
    material.set_uniform(
        "FogRange",
        Vec2::from_array(params.fog_range(56 * 128, 512.0)) / 128.0,
    );
    material.set_uniform(
        "FogColor",
        Vec3::from_array(rs09_client::atmosphere::rgb(params.fog_color)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_shader_has_the_material_code_at_every_marker() {
        let ShaderSource::Glsl { vertex, fragment } = world_shader() else {
            unreachable!()
        };
        for source in [vertex, fragment] {
            assert!(!source.contains("//@material"), "a marker was left");
            assert!(!source.contains('@'), "a constant was left unfilled");
        }
        // The stages call what the material code defines.
        assert!(
            vertex.contains("void materialVertex(){") && vertex.contains(" materialVertex();\n}")
        );
        for call in [
            "scroll*=materialScroll(id);",
            "tex=materialMip(tex,point,origin,info.rg,size);",
            "result=materialSurface(result,tex);",
            "gl_FragColor=materialOutput(result);",
        ] {
            assert!(fragment.contains(call), "{call}");
        }
        for definition in [
            "float materialScroll(float id){",
            "vec4 materialMip(vec4 base,vec2 point,vec2 origin,vec2 wrap,float size){",
            "vec4 materialSurface(vec4 result,vec4 tex){",
            "vec4 materialOutput(vec4 result){",
        ] {
            assert!(fragment.contains(definition), "{definition}");
        }
        // `#extension` has to precede every other token but `#version`.
        assert!(fragment.starts_with("#version 100\n#ifdef GL_OES_standard_derivatives\n"));
        // Both stages declare the varyings the materials add.
        let varyings = "varying highp vec3 worldPoint;varying highp vec3 worldNormal;";
        assert!(vertex.contains(varyings) && fragment.contains(varyings));
    }
}
