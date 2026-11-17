//! The HD material renderers of `MaterialManager`: which material a texture
//! selects, the tables the renderers sample, their coordinate and colour
//! formulas, and the GLSL that evaluates them in the world shader.
//!
//! The original binds one fixed-function setup per material
//! (`SpecularMaterialRenderer`, `LiquidMaterialRenderer`,
//! `WaterMaterialRenderer`, `WaterfallMaterialRenderer`,
//! `UnlitMaterialRenderer`) before drawing the faces of a texture. Here the
//! material type and its argument are looked up per vertex and per fragment
//! from the face's texture ID, and one shader branches on them.
//!
//! The plain functions in this module state each formula once on the CPU; the
//! GLSL below repeats them, and `tests/materials.rs` pins the functions to
//! values worked out from the Java. The shader itself is only exercised by
//! captures.
//!
//! What is exact and what is emulated is listed per material at
//! [`VERTEX_GLSL`] and [`FRAGMENT_GLSL`].
use crate::texture::Material;
use std::sync::atomic::{AtomicBool, Ordering};

pub mod noise;

/// `MaterialManager.renderers` indices.
pub const SPECULAR: u8 = 1;
pub const LIQUID: u8 = 2;
/// Bound explicitly by the underwater pass, never through a texture.
pub const UNDERWATER: u8 = 3;
pub const WATER: u8 = 4;
pub const WATERFALL: u8 = 5;
pub const UNLIT: u8 = 6;

/// `GlobalConfig.VIEW_DISTANCE` and `VIEW_FADE_DISTANCE` at the pinned
/// `TILE_DISTANCE` of 56.
pub const VIEW_DISTANCE: i32 = 56 * 128;
pub const VIEW_FADE_DISTANCE: f32 = 56.0 / 28.0 * 256.0;

static HIGH_WATER_DETAIL: AtomicBool = AtomicBool::new(true);
static UNDERWATER_PASS: AtomicBool = AtomicBool::new(false);

/// `Preferences.highWaterDetail`, read when the frame's uniforms are filled.
pub fn set_high_water_detail(high: bool) {
    HIGH_WATER_DETAIL.store(high, Ordering::Relaxed);
}
pub fn high_water_detail() -> bool {
    HIGH_WATER_DETAIL.load(Ordering::Relaxed)
}
/// Whether an underwater scene is drawn before the surface. Without one the
/// water material composites over [`water_backdrop`] itself; with one it
/// leaves its translucent colour to the blend, as the original does.
pub fn set_underwater_pass(drawn: bool) {
    UNDERWATER_PASS.store(drawn, Ordering::Relaxed);
}
pub fn underwater_pass() -> bool {
    UNDERWATER_PASS.load(Ordering::Relaxed)
}

/// `MaterialManager.setMaterial`: without high water detail the water
/// material is the liquid material with argument 2.
pub fn route(material_type: u8, argument: u8, high_water_detail: bool) -> (u8, u8) {
    if material_type == WATER && !high_water_detail {
        (LIQUID, 2)
    } else {
        (material_type, argument)
    }
}

/// `MaterialRenderer.getFlags`: 1 keeps the texture from being bound, 2 its
/// colour combine mode, 4 its alpha combine mode and 8 its scroll
/// (`GlTexture.bind`).
pub fn flags(material_type: u8) -> u8 {
    match material_type {
        SPECULAR => 4,
        WATER => 15,
        _ => 0,
    }
}

/// `Camera.cameraPitch` of a view direction given with height pointing up:
/// 2048 units to the turn, positive looking down. The scene camera is built
/// from whole units, which rounding recovers.
pub fn camera_pitch(forward: [f32; 3]) -> i32 {
    let [x, y, z] = forward.map(f64::from);
    let length = (x * x + y * y + z * z).sqrt();
    if length == 0.0 {
        return 0;
    }
    ((-y / length).clamp(-1.0, 1.0).asin() * 2048.0 / std::f64::consts::TAU).round() as i32
}

/// `WaterMaterialRenderer.bind`: the eye depth where the water starts to
/// fade to its tile's colour, and the depth where it has faded.
pub fn water_range(pitch: i32) -> [f32; 2] {
    let mut near = 2662.4001f32;
    near += (pitch - 128) as f32 * 0.5f32;
    let far = VIEW_DISTANCE as f32 - VIEW_FADE_DISTANCE;
    if near >= far {
        near = far - 1.0;
    }
    [near, far]
}

/// The `MaterialView` uniform of the surface: the near depth of
/// [`water_range`], high water detail, a one enabling the materials (interface
/// models leave the uniform zero), and whether the water composites over its
/// own backdrop. The underwater pass sets the last to [`UNDERWATER`] instead,
/// which puts every face under that material, with its fog range and argument
/// in the first two (`underwater::uniforms`).
pub fn view(forward: [f32; 3]) -> [f32; 4] {
    [
        water_range(camera_pitch(forward))[0],
        f32::from(u8::from(high_water_detail())),
        1.0,
        f32::from(u8::from(!underwater_pass())),
    ]
}

/// The two-texel alpha texture of `WaterMaterialRenderer.createAlphaTexture`
/// (0 and 255, linear, clamped to the edge) at coordinate `s`.
pub fn water_ramp(s: f32) -> f32 {
    (s * 2.0 - 0.5).clamp(0.0, 1.0)
}

/// How far the water has faded at an eye depth: the eye-linear S coordinate
/// `(depth - near) / (far - near)` through [`water_ramp`].
pub fn water_fade(depth: f32, [near, far]: [f32; 2]) -> f32 {
    // The planes are `1 / (near - far)` on eye Z and `near / (near - far)`.
    let s = -depth * (1.0 / (near - far)) + near / (near - far);
    water_ramp(s)
}

/// `WaterMaterialRenderer.waterSpecularColor`, the constant added to the noise.
pub const WATER_SPECULAR: [f32; 4] = [0.1, 0.1, 0.15, 0.1];
/// `WaterMaterialRenderer.waterColor` before any tile has set it.
pub const WATER_DEFAULT: [f32; 3] = [0.073, 0.169, 0.24];

/// The water material's colour for a filtered sample of the ridged volume
/// (luminance, alpha in 0 to 1), its tile's water colour and a fade.
///
/// Unit 0 adds the constant to the texture and doubles the colour; unit 1
/// interpolates toward the water colour and full opacity by the ramp.
pub fn water_surface([luminance, alpha]: [f32; 2], water: [f32; 3], fade: f32) -> [f32; 4] {
    let mix = |from: f32, to: f32| to * fade + from * (1.0 - fade);
    let unit0 = |constant: f32| ((luminance + constant) * 2.0).clamp(0.0, 1.0);
    [
        mix(unit0(WATER_SPECULAR[0]), water[0]),
        mix(unit0(WATER_SPECULAR[1]), water[1]),
        mix(unit0(WATER_SPECULAR[2]), water[2]),
        mix((alpha + WATER_SPECULAR[3]).clamp(0.0, 1.0), 1.0),
    ]
}

/// The fog start of `UnderwaterMaterialRenderer.applyFogFade`, which paints
/// the water tiles in their plain colour before the underwater scene.
pub const BACKDROP_FOG_START: f32 = VIEW_DISTANCE as f32 - VIEW_FADE_DISTANCE * 2.0;

/// The colour `applyFogFade` leaves under a water tile at an eye depth.
pub fn water_backdrop(water: [f32; 3], fog: [f32; 3], depth: f32) -> [f32; 3] {
    let end = VIEW_DISTANCE as f32 - VIEW_FADE_DISTANCE;
    let factor = ((end - depth) / (end - BACKDROP_FOG_START)).clamp(0.0, 1.0);
    std::array::from_fn(|i| fog[i] + (water[i] - fog[i]) * factor)
}

/// A water tile's colour in the texture coordinate of its vertices, which the
/// water and liquid materials do not otherwise read: red and green in X,
/// blue in Y.
pub fn water_uv(color: u32) -> [f32; 2] {
    [(color >> 8 & 0xffff) as f32, (color & 0xff) as f32]
}

/// `LiquidMaterialRenderer.setArgument`: the turbulence scale of bits 0-1.
pub fn liquid_turbulence(argument: u8) -> f32 {
    match argument & 3 {
        2 => 0.05,
        3 => 0.1,
        _ => 0.025,
    }
}
/// Bit 0x40: the vertex colour is not multiplied by the ambient light.
pub fn liquid_full_bright(argument: u8) -> bool {
    argument & 0x40 != 0
}
/// Bit 0x80: the noise volume is not added.
pub fn liquid_plain(argument: u8) -> bool {
    argument & 0x80 != 0
}

/// The liquid vertex program's table columns for a world position in fine
/// units: `floor(frac((x + z) / 8192) * 64)` and the same of `z - x`.
pub fn liquid_columns(x: f32, z: f32) -> [usize; 2] {
    [x + z, z - x].map(|n| {
        // 0.0001220703125 in the program.
        let n = n * (1.0 / 8192.0);
        ((n - n.floor()) * 64.0).floor() as usize
    })
}

/// The liquid vertex program's texture coordinate 0: the position over 128
/// plus the table's offsets for the clock's row.
pub fn liquid_texcoord(table: &[i32], clock: i32, argument: u8, x: f32, z: f32) -> [f32; 2] {
    let row = (clock & 0xff) as usize * noise::LIQUID_COLUMNS;
    let [s, t] = liquid_columns(x, z);
    let turbulence = liquid_turbulence(argument);
    [
        x * 0.0078125 + table[row + s] as f32 / 4096.0 * turbulence,
        z * 0.0078125 + table[row + t] as f32 / 4096.0 * turbulence,
    ]
}

/// `GlRenderer.setPerspectiveFrustum`'s Z row at near 50 and the view
/// distance: the liquid vertex program fogs by clip Z instead of eye depth.
/// (The original's row also carries the depth layer of the draw, which this
/// renderer does not have.)
pub fn liquid_fog_row() -> [f32; 2] {
    let (near, far) = (50.0f32, VIEW_DISTANCE as f32);
    [
        -(far + near) / (far - near),
        -(near * 2.0 * far) / (far - near),
    ]
}
pub fn liquid_fog_coordinate(depth: f32) -> f32 {
    let [scale, offset] = liquid_fog_row();
    scale * -depth + offset
}

/// `WaterfallMaterialRenderer.setArgument`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Waterfall {
    /// R advances by this per client cycle.
    pub animation_speed: f32,
    /// T advances by this per client cycle.
    pub scroll_speed: f32,
    /// S and T per fine unit.
    pub scale: f32,
    /// S follows X instead of Z.
    pub horizontal: bool,
}
pub fn waterfall(argument: u8) -> Waterfall {
    Waterfall {
        animation_speed: ((argument >> 3 & 3) + 1) as f32 * 0.01f32,
        scroll_speed: -0.01f32 * ((argument & 3) + 1) as f32,
        scale: if argument & 0x40 == 0 {
            4.8828125e-4
        } else {
            9.765625e-4
        },
        horizontal: argument & 0x80 != 0,
    }
}
impl Waterfall {
    /// The noise coordinate of a point in fine units, `y` being the
    /// original's height (negative upward).
    pub fn coordinate(&self, [x, y, z]: [f32; 3], clock: i32) -> [f32; 3] {
        [
            self.scale * if self.horizontal { x } else { z },
            self.scale * y + self.scroll_speed * clock as f32,
            clock as f32 * self.animation_speed,
        ]
    }
}

/// `SpecularMaterialRenderer`: the cube map of argument 1 holds the 96th
/// power, 2 the 36th and 3 the 12th.
pub fn specular_exponent(argument: u8) -> f64 {
    match argument {
        1 => 96.0,
        2 => 36.0,
        _ => 12.0,
    }
}
/// One cube-map texel of `createCubeMapTextures` with three or more texture
/// units, for the Z component of its direction after normalising.
pub fn specular_texel(z: f32, exponent: f64) -> u8 {
    if z > 0.0 {
        ((f64::from(z).powf(exponent) * 255.0) as i32 / 2) as u8
    } else {
        0
    }
}
/// The direction component the cube maps store: `createCubeMapTextures` gives
/// each texel of each face the Z of its normalised direction.
pub fn specular_cube_z(face: usize, column: usize, row: usize) -> f32 {
    let u = column as f32 * 2.0 / 64.0 - 1.0;
    let v = row as f32 * 2.0 / 64.0 - 1.0;
    let inverse = (1.0 / f64::from(u * u + v * v + 1.0).sqrt()) as f32;
    match face {
        0 => -(u * inverse),
        1 => u * inverse,
        2 => v * inverse,
        3 => -(v * inverse),
        4 => inverse,
        _ => -inverse,
    }
}
/// The texture matrix of the cube map unit turns the eye-space normal 22.5
/// degrees about X, so the looked-up Z is this combination of the normal's
/// eye Y (up the screen) and eye Z (toward the viewer).
pub fn specular_direction([_, y, z]: [f32; 3]) -> f32 {
    let angle = 22.5f32.to_radians();
    y * angle.sin() + z * angle.cos()
}

/// Width and height of the tables texture.
pub const TABLES: [usize; 2] = [512, 1024];
/// The texture every material samples besides the atlas, as RGBA rows:
///
/// - rows 0 to 511: the 64 slices of the noise volumes, eight to a row; red
///   and green are the ridged volume's luminance and alpha, blue the fractal
///   volume's value;
/// - rows 512 to 767: the liquid turbulence table, one clock row each, 64
///   columns; red-green and blue-alpha are the S and T offsets plus 32768,
///   high byte first;
/// - rows 768 and 769: per texture ID, the material type in red, its argument
///   in green and the texture's mipmap mode in blue.
pub fn tables(materials: &[Option<Material>], mipmaps: &[u8]) -> Vec<u8> {
    let [width, height] = TABLES;
    let mut out = vec![0u8; width * height * 4];
    let (ridged, fractal) = (noise::ridged(), noise::fractal());
    for z in 0..noise::SIDE {
        for y in 0..noise::SIDE {
            for x in 0..noise::SIDE {
                let from = ((z * noise::SIDE + y) * noise::SIDE + x) * 2;
                let to = ((z / 8 * 64 + y) * width + z % 8 * 64 + x) * 4;
                out[to..to + 4].copy_from_slice(&[
                    ridged[from],
                    ridged[from + 1],
                    fractal[from],
                    255,
                ]);
            }
        }
    }
    let liquid = noise::liquid();
    for row in 0..noise::LIQUID_ROWS {
        for col in 0..noise::LIQUID_COLUMNS {
            // The renderer's S and T tables are two runs of the same generator.
            let value = ((liquid[row * noise::LIQUID_COLUMNS + col] + 32768) as u16).to_be_bytes();
            let to = ((512 + row) * width + col) * 4;
            out[to..to + 4].copy_from_slice(&[value[0], value[1], value[0], value[1]]);
        }
    }
    for (id, material) in materials.iter().enumerate().take(width * 2) {
        let Some(material) = material else {
            continue;
        };
        let to = ((768 + id / width) * width + id % width) * 4;
        out[to..to + 4].copy_from_slice(&[
            material.material_type,
            material.animation_type,
            mipmaps.get(id).copied().unwrap_or(0),
            255,
        ]);
    }
    out
}

/// Stored mipmap levels below the base: `GlTexture.bind` allocates four
/// levels for mipmap mode 2.
pub const MIP_LEVELS: usize = 3;
/// The mip atlas holds one 128 by 64 cell per texture, in the atlas's order.
pub const MIP_CELL: [usize; 2] = [128, 64];
/// Column of a level (1 to 3) inside its texture's cell.
pub fn mip_column(level: usize) -> usize {
    [0, 64, 96][level - 1]
}
/// The next level of a square RGBA image: each texel the rounded mean of
/// four. `glGenerateMipmap` leaves the filter to the implementation; this is
/// the usual box.
pub fn halve(pixels: &[u8], size: usize) -> Vec<u8> {
    let half = size / 2;
    let mut out = vec![0u8; half * half * 4];
    for y in 0..half {
        for x in 0..half {
            for c in 0..4 {
                let at = |dx: usize, dy: usize| {
                    u32::from(pixels[((y * 2 + dy) * size + x * 2 + dx) * 4 + c])
                };
                out[(y * half + x) * 4 + c] =
                    ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4) as u8;
            }
        }
    }
    out
}

/// Levels 1 to 3 of every texture, laid out for the shader's `materialMip`.
pub struct MipAtlas {
    grid: usize,
    pub rgba: Vec<u8>,
}
impl MipAtlas {
    /// For an atlas of `grid` by `grid` textures.
    pub fn new(grid: usize) -> Self {
        Self {
            grid,
            rgba: vec![0; grid * MIP_CELL[0] * grid * MIP_CELL[1] * 4],
        }
    }
    pub fn size(&self) -> [usize; 2] {
        [self.grid * MIP_CELL[0], self.grid * MIP_CELL[1]]
    }
    /// Stores the levels of texture `id` from its base level of `size`
    /// pixels a side, as `GlTexture.bind` builds them: mode 2 filters each
    /// level from the one above, mode 1 generates the texture again at each
    /// halved size (`regenerate`), and any other mode has no further levels.
    ///
    /// Mode 1 continues down to one pixel in the original; only three levels
    /// are kept here. No texture of the pinned cache uses mode 1.
    pub fn add(
        &mut self,
        id: usize,
        base: &[u8],
        size: usize,
        mode: u8,
        mut regenerate: impl FnMut(usize) -> Result<Vec<u8>, String>,
    ) -> Result<(), String> {
        if !matches!(mode, 1 | 2) || id >= self.grid * self.grid {
            return Ok(());
        }
        let width = self.size()[0];
        let origin = [id % self.grid * MIP_CELL[0], id / self.grid * MIP_CELL[1]];
        let mut level_pixels = base.to_vec();
        for level in 1..=MIP_LEVELS {
            let side = size >> level;
            level_pixels = if mode == 1 {
                regenerate(side)?
            } else {
                halve(&level_pixels, side * 2)
            };
            for y in 0..side {
                let to = ((origin[1] + y) * width + origin[0] + mip_column(level)) * 4;
                self.rgba[to..to + side * 4]
                    .copy_from_slice(&level_pixels[y * side * 4..(y + 1) * side * 4]);
            }
        }
        Ok(())
    }
}

/// A float as a GLSL ES literal.
fn literal(value: f32) -> String {
    let text = format!("{value:?}");
    if text.contains(['.', 'e']) {
        text
    } else {
        format!("{text}.0")
    }
}

/// Fills the `@NAME@` constants of a snippet from the functions above.
fn constants(snippet: &str) -> String {
    let [fog_scale, fog_offset] = liquid_fog_row();
    let far = VIEW_DISTANCE as f32 - VIEW_FADE_DISTANCE;
    let angle = 22.5f32.to_radians();
    let vec3 = |v: [f32; 3]| {
        format!(
            "vec3({},{},{})",
            literal(v[0]),
            literal(v[1]),
            literal(v[2])
        )
    };
    [
        // Eye depth is measured in tiles in the shader.
        ("@LIQUID_FOG_SCALE@", literal(-fog_scale)),
        ("@LIQUID_FOG_OFFSET@", literal(-fog_offset / 128.0)),
        ("@WATER_FAR@", literal(far)),
        (
            "@WATER_SPECULAR@",
            vec3([WATER_SPECULAR[0], WATER_SPECULAR[1], WATER_SPECULAR[2]]),
        ),
        ("@WATER_SPECULAR_ALPHA@", literal(WATER_SPECULAR[3])),
        ("@WATER_DEFAULT@", vec3(WATER_DEFAULT)),
        ("@BACKDROP_FOG_START@", literal(BACKDROP_FOG_START / 128.0)),
        ("@SPECULAR_SIN@", literal(angle.sin())),
        ("@SPECULAR_COS@", literal(angle.cos())),
    ]
    .iter()
    .fold(snippet.to_owned(), |text, (name, value)| {
        text.replace(name, value)
    })
}

/// Vertex-stage material code, placed before `main`, which ends by calling
/// `materialVertex()`.
///
/// - Unlit (6): the vertex colour is passed unlit. Exact. (The original
///   disables GL lighting, which also drops its point lights; the terrain's
///   light meshes are separate geometry there and stay here.)
/// - Liquid (2): the ARB vertex program. Texture coordinate 0 is the world
///   position over 128 plus the turbulence table's offsets, the colour is the
///   vertex colour times the ambient light (or one) with alpha one, and the
///   fog coordinate is clip Z. The table lookup, the arithmetic and the row
///   selection are the program's. World-absolute coordinates stand in for the
///   original's scene-local ones, which shifts the table by a multiple of
///   eight columns whenever the scene base is not a multiple of 64 tiles.
///   The fog coordinate omits the draw's depth-layer term.
/// - Water (4): only routes the tile's water colour to the fragment stage.
/// - Underwater (3), on every face while `MaterialView.w` is 3
///   (`MaterialManager.renderingUnderwater`): the coordinate of the
///   material's gradient. A floor vertex brings it in its texture coordinate
///   (`GlTile`'s `vertexUnderwaterCoord`) and takes its texture coordinate
///   from its position and the group's scale, carried where scenery has its
///   light key; scenery takes `(y + argument) / range`, the unit's eye-linear
///   S plane. World-absolute floor coordinates stand in for scene-local
///   ones, as on the surface.
pub const VERTEX_GLSL: &str = r#"varying highp vec3 worldPoint;varying highp vec3 worldNormal;varying highp float underDepth;
uniform sampler2D MaterialTables;uniform float TextureTime;uniform vec4 MaterialView;
vec2 materialKind(float id){
 if(MaterialView.z<0.5||MaterialView.w>2.5)return vec2(0.0);
 vec2 k=floor(texture2D(MaterialTables,vec2((mod(id,512.0)+0.5)/512.0,(floor(id/512.0)+768.5)/1024.0)).rg*255.0+vec2(0.5));
 if(k.x==4.0&&MaterialView.y<0.5)k=vec2(2.0);
 return k;
}
void materialVertex(){
 worldPoint=position;worldNormal=LightRegion.z<0.0?vec3(0.0):normal.xyz;underDepth=0.0;
 if(MaterialView.w>2.5){
  if(mod(normal.w,2048.0)>=1024.0){underDepth=texcoord.x;uv=(position.xz+vec2(3200.0))*128.0/floor(normal.w/8192.0);}
  else underDepth=(MaterialView.y-position.y*128.0)/MaterialView.x;
 }
 vec2 k=materialKind(mod(floor(normal.w+0.5),1024.0));
 if(k.x==6.0)color=vec4(clamp(color0.rgb/255.0,0.0,1.0),color0.a/255.0);
 if(k.x==4.0)color=mod(normal.w,2048.0)>=1024.0?vec4(floor(texcoord.x/256.0),mod(texcoord.x,256.0),texcoord.y,255.0)/255.0:vec4(@WATER_DEFAULT@,1.0);
 if(k.x==2.0){
  float turbulence=mod(k.y,4.0);turbulence=turbulence==2.0?0.05:(turbulence==3.0?0.1:0.025);
  vec3 ambient=mod(floor(k.y/64.0),2.0)==0.0?AmbientLight:vec3(1.0);
  vec2 world=(position.xz+vec2(3200.0))*128.0;
  vec2 column=floor(fract(vec2(world.x+world.y,world.y-world.x)*0.0001220703125)*64.0);
  float row=(mod(floor(TextureTime),256.0)+512.5)/1024.0;
  float s=pair(texture2D(MaterialTables,vec2((column.x+0.5)/512.0,row)).rg)-32768.0;
  float t=pair(texture2D(MaterialTables,vec2((column.y+0.5)/512.0,row)).ba)-32768.0;
  uv=world*0.0078125+vec2(s,t)/4096.0*turbulence;
  color=vec4(clamp(color0.rgb/255.0*ambient,0.0,1.0),1.0);
  fogDistance=abs(dot(position-CameraPosition,CameraForward)*@LIQUID_FOG_SCALE@-@LIQUID_FOG_OFFSET@);
 }
}
"#;

/// The first lines of the fragment shader: derivatives, where the context
/// has them, select the mipmap level.
pub const FRAGMENT_HEADER_GLSL: &str = r#"#ifdef GL_OES_standard_derivatives
#extension GL_OES_standard_derivatives : enable
#define MATERIAL_MIPMAPS 1
#endif
"#;

/// Fragment-stage material code, placed before `main`.
///
/// `materialScroll` runs first and reads the face's material.
/// `materialMip` replaces the base texel with the trilinear sample of
/// `GL_LINEAR_MIPMAP_LINEAR` over four levels where the texture's mipmap mode
/// asks for it and the context has derivatives. `materialSurface` applies the
/// material after the texture's own combine mode, and `materialOutput` runs
/// after fog.
///
/// - Noise volumes: 64 slices in a 2D texture with nearest sampling, filtered
///   here over the eight neighbours with repeat on all three axes, as
///   `GL_LINEAR` on a 3D texture does (in float, where a GPU filters with
///   fixed-point weights). The original's fallback without 3D textures steps
///   through whole slices instead; that path is not reproduced.
/// - Specular (1): `rgb += cube(normal).a * texture.a`, the alpha staying the
///   vertex alpha. The cube maps hold `pow(z, 96 | 36 | 12) * 255 / 2` in
///   bytes for the direction's Z, and their unit's texture matrix turns the
///   eye normal 22.5 degrees about X; the power, both integer divisions and
///   the rotation are evaluated per fragment. The 64-texel faces' bilinear
///   filtering is not reproduced. Without high detail lighting the renderer
///   binds no cube map (`SpecularMaterialRenderer.bind` only sets the alpha
///   source), so nothing is added: the vertex stage then passes no normal
///   (`sd_light::light_region`).
/// - Liquid (2): unless bit 0x80 is set, adds the ridged volume's luminance at
///   (x / 1024, z / 1024, clock * 0.005). The texture is sampled at the
///   vertex program's coordinate without the texture's scroll, which a vertex
///   program bypasses.
/// - Water (4): unit 0 is `(noise + (0.1, 0.1, 0.15)) * 2` with alpha
///   `noise.a + 0.1`, the noise at (x / 1024, z / 1024, clock * 0.005 or 0),
///   the material's own texture and the vertex colour unused; unit 1
///   interpolates colour toward the tile's water colour and alpha toward one
///   by the two-texel ramp over eye depth. Then, when no underwater scene is
///   drawn, the result is composited over the plain fogged water colour that
///   `UnderwaterMaterialRenderer.applyFogFade` paints first; the underwater
///   terrain that belongs between the two is not drawn.
/// - Underwater (3), on every face while `MaterialView.w` is 3, after the
///   texture's own stage and instead of the texture's own material: unit 1
///   adds the ridged volume's luminance at (0.001 x + 0.0009 y,
///   0.0009 y + 0.001 z, clock * 0.005); unit 2 interpolates the colour
///   toward the water fog colour (the pass's `FogColor`) by the eight-texel
///   gradient, whose linear lookup is the closed form used here
///   (`underwater::ramp`). Alpha passes through. World-absolute x and z
///   stand in for scene-local ones, a phase of the animated noise.
/// - Waterfall (5): the texture stage's colour and alpha times the fractal
///   volume at S = scale * (X or Z), T = scale * Y - 0.01 * n * clock,
///   R = clock * 0.01 * m. World-absolute coordinates stand in for
///   scene-local ones (a half-period shift in S at the smaller scale when
///   the scene base is an odd multiple of eight tiles).
///
/// The clock is the whole client cycle. Products of the clock are reduced by
/// their exact period before use; the original multiplies in single
/// precision without reducing.
pub const FRAGMENT_GLSL: &str = r#"varying highp vec3 worldPoint;varying highp vec3 worldNormal;varying highp float underDepth;
uniform sampler2D MaterialTables;uniform sampler2D MipAtlas;uniform vec4 MaterialView;uniform vec3 CameraForward;
vec2 materialKind;float materialMode;vec4 materialWater;
vec4 volumeTexel(vec3 c){
 c=mod(c,64.0);
 return texture2D(MaterialTables,(vec2(mod(c.z,8.0),floor(c.z/8.0))*64.0+c.xy+vec2(0.5))/vec2(512.0,1024.0));
}
vec4 volume(vec3 coordinate){
 vec3 p=coordinate*64.0-vec3(0.5);vec3 b=floor(p);vec3 f=p-b;
 vec4 front=mix(mix(volumeTexel(b),volumeTexel(b+vec3(1.0,0.0,0.0)),f.x),mix(volumeTexel(b+vec3(0.0,1.0,0.0)),volumeTexel(b+vec3(1.0,1.0,0.0)),f.x),f.y);
 vec4 back=mix(mix(volumeTexel(b+vec3(0.0,0.0,1.0)),volumeTexel(b+vec3(1.0,0.0,1.0)),f.x),mix(volumeTexel(b+vec3(0.0,1.0,1.0)),volumeTexel(b+vec3(1.0,1.0,1.0)),f.x),f.y);
 return mix(front,back,f.z);
}
float materialScroll(float id){
 materialKind=vec2(0.0);materialMode=0.0;materialWater=vec4(0.0);
 if(MaterialView.z<0.5)return 1.0;
 vec3 k=floor(texture2D(MaterialTables,vec2((mod(id,512.0)+0.5)/512.0,(floor(id/512.0)+768.5)/1024.0)).rgb*255.0+vec3(0.5));
 materialKind=k.xy;materialMode=k.z;
 if(k.x==4.0&&MaterialView.y<0.5)materialKind=vec2(2.0);
 if(MaterialView.w>2.5)materialKind=vec2(0.0);
 return materialKind.x==2.0||materialKind.x==4.0?0.0:1.0;
}
#ifdef MATERIAL_MIPMAPS
vec4 mipTexel(vec2 pixel,vec2 cell,vec2 wrap,float side){
 vec2 coord=mix(clamp(pixel,vec2(0.0),vec2(side-1.0)),mod(pixel,vec2(side)),wrap);
 return texture2D(MipAtlas,(cell+coord+vec2(0.5))/vec2(4096.0,2048.0));
}
vec4 mipSample(vec2 point,vec2 cell,vec2 wrap,float size,float level){
 float scale=exp2(level);float side=size/scale;
 cell.x+=level<1.5?0.0:(level<2.5?64.0:96.0);
 vec2 p=(point+vec2(0.5))/scale-vec2(0.5);vec2 b=floor(p);vec2 f=p-b;
 return mix(mix(mipTexel(b,cell,wrap,side),mipTexel(b+vec2(1.0,0.0),cell,wrap,side),f.x),mix(mipTexel(b+vec2(0.0,1.0),cell,wrap,side),mipTexel(b+vec2(1.0,1.0),cell,wrap,side),f.x),f.y);
}
vec4 materialMip(vec4 base,vec2 point,vec2 origin,vec2 wrap,float size){
 vec2 dx=dFdx(point);vec2 dy=dFdy(point);
 float lod=clamp(0.5*log2(max(max(dot(dx,dx),dot(dy,dy)),1e-12)),0.0,3.0);
 if(materialMode<0.5||lod<=0.0)return base;
 vec2 cell=vec2(origin.x,origin.y*0.5);float lower=floor(lod);
 vec4 fine=lower<0.5?base:mipSample(point,cell,wrap,size,lower);
 if(lod==lower)return fine;
 return mix(fine,mipSample(point,cell,wrap,size,lower+1.0),lod-lower);
}
#else
vec4 materialMip(vec4 base,vec2 point,vec2 origin,vec2 wrap,float size){return base;}
#endif
vec4 materialSurface(vec4 result,vec4 tex){
 float kind=materialKind.x;float arg=materialKind.y;float clock=floor(TextureTime);
 if(kind==1.0&&dot(worldNormal,worldNormal)>0.0){
  vec3 forward=normalize(CameraForward);vec3 up=normalize(vec3(0.0,1.0,0.0)-forward*forward.y);
  float z=dot(normalize(worldNormal),up*@SPECULAR_SIN@-forward*@SPECULAR_COS@);
  float power=arg==1.0?96.0:(arg==2.0?36.0:12.0);
  float cube=z>0.0?floor(floor(pow(z,power)*255.0)/2.0)/255.0:0.0;
  result.rgb=clamp(result.rgb+vec3(cube*tex.a),0.0,1.0);
 }
 if(kind==2.0&&mod(floor(arg/128.0),2.0)==0.0)result.rgb=clamp(result.rgb+vec3(volume(vec3(worldPoint.xz*0.125,mod(clock,200.0)*0.005)).r),0.0,1.0);
 if(kind==4.0){
  vec4 ridged=volume(vec3(worldPoint.xz*0.125,mod(arg,2.0)==1.0?mod(clock,200.0)*0.005:0.0));
  float depth=fogDistance*128.0;
  float fade=clamp((depth-MaterialView.x)/(@WATER_FAR@-MaterialView.x)*2.0-0.5,0.0,1.0);
  result=vec4(mix(clamp((vec3(ridged.r)+@WATER_SPECULAR@)*2.0,0.0,1.0),color.rgb,fade),mix(clamp(ridged.g+@WATER_SPECULAR_ALPHA@,0.0,1.0),1.0,fade));
  if(MaterialView.w>0.5)materialWater=vec4(mix(FogColor,color.rgb,clamp((FogRange.y-fogDistance)/(FogRange.y-@BACKDROP_FOG_START@),0.0,1.0)),1.0);
 }
 if(kind==5.0){
  float scale=mod(floor(arg/64.0),2.0)==0.0?0.0625:0.125;
  vec3 at=vec3((arg>=128.0?worldPoint.x:worldPoint.z)*scale,-worldPoint.y*scale-mod((mod(arg,4.0)+1.0)*clock,100.0)*0.01,mod((mod(floor(arg/8.0),4.0)+1.0)*clock,100.0)*0.01);
  result*=volume(at).b;
 }
 if(MaterialView.w>2.5){
  vec3 at=vec3(worldPoint.x+3200.0,-worldPoint.y,worldPoint.z+3200.0)*128.0;
  result.rgb=clamp(result.rgb+vec3(volume(vec3(0.001*at.x+0.0009*at.y,0.0009*at.y+0.001*at.z,mod(clock,200.0)*0.005)).r),0.0,1.0);
  result.rgb=mix(result.rgb,FogColor,(115.0+20.0*clamp(underDepth*8.0-0.5,0.0,7.0))/255.0);
 }
 return result;
}
vec4 materialOutput(vec4 result){
 return materialWater.a>0.5?vec4(mix(materialWater.rgb,result.rgb,result.a),1.0):result;
}
"#;

/// The world shader's sources with the material code in place. The template
/// marks where it goes with the lines `//@material-vertex`,
/// `//@material-header` and `//@material-fragment`.
pub fn world_shader(vertex: &str, fragment: &str) -> &'static [String; 2] {
    static SOURCES: std::sync::OnceLock<[String; 2]> = std::sync::OnceLock::new();
    SOURCES.get_or_init(|| {
        [
            vertex.replace("//@material-vertex\n", &constants(VERTEX_GLSL)),
            fragment
                .replace("//@material-header\n", FRAGMENT_HEADER_GLSL)
                .replace("//@material-fragment\n", &constants(FRAGMENT_GLSL)),
        ]
    })
}
