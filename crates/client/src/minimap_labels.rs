//! GPU presentation of the minimap's text labels, clipped by the component
//! mask as `GlFont.setLineMask` clips them. Layout is `minimap::labels`.
use macroquad::prelude::*;
use rs09_client::{
    minimap::{
        LabelFont, View,
        labels::{self, Draw},
    },
    pack::Pack,
    world_map::MapElementList,
};
use std::collections::BTreeMap;

struct Atlas {
    texture: Texture2D,
    cell: [usize; 2],
    size: [f32; 2],
}
impl Atlas {
    /// The 256 glyphs in a 16 by 16 grid; palette index 0 is transparent.
    fn new(sheet: &rs09_client::ui::sprite::Sheet) -> Self {
        let extent = |axis: usize| {
            sheet
                .frames
                .iter()
                .take(256)
                .map(|f| f.size[axis])
                .max()
                .unwrap_or(0)
                .max(1)
                + 2
        };
        let cell = [extent(0), extent(1)];
        let (w, h) = (cell[0] * 16, cell[1] * 16);
        let mut pixels = vec![0; w * h * 4];
        for (i, f) in sheet.frames.iter().take(256).enumerate() {
            let (ox, oy) = (i % 16 * cell[0], i / 16 * cell[1]);
            for y in 0..f.size[1] {
                for x in 0..f.size[0] {
                    let alpha = if f.indices[y * f.size[0] + x] != 0 {
                        255
                    } else {
                        0
                    };
                    let at = ((y + oy) * w + x + ox) * 4;
                    pixels[at..at + 4].copy_from_slice(&[255, 255, 255, alpha]);
                }
            }
        }
        let texture = Texture2D::from_rgba8(w as u16, h as u16, &pixels);
        texture.set_filter(FilterMode::Nearest);
        Self {
            texture,
            cell,
            size: [w as f32, h as f32],
        }
    }
}

enum Fonts {
    Unloaded,
    Loaded(Box<([LabelFont; 3], [Atlas; 3])>),
    Failed,
}

pub struct Text {
    material: Material,
    fonts: Fonts,
}
impl Text {
    pub fn new() -> Result<Self, String> {
        use macroquad::miniquad::{
            BlendFactor, BlendState, BlendValue, Equation, PipelineParams, UniformDesc, UniformType,
        };
        let material = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX,
                fragment: FRAGMENT,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    )),
                    ..Default::default()
                },
                textures: vec!["MaskTexture".into()],
                uniforms: vec![UniformDesc::new("MaskBounds", UniformType::Float4)],
            },
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            material,
            fonts: Fonts::Unloaded,
        })
    }

    /// `Fonts.p11Full`, `p12Full` and `b12Full`: metrics from the interface
    /// pack, glyph sprites from the visual pack.
    fn load(ui: &Pack, visual: &Pack) -> Result<([LabelFont; 3], [Atlas; 3]), String> {
        let ids: BTreeMap<String, i32> =
            serde_json::from_slice(ui.get("overhead/assets")?).map_err(|e| e.to_string())?;
        let mut fonts = Vec::new();
        let mut atlases = Vec::new();
        for name in ["p11_full", "p12_full", "b12_full"] {
            let id = ids
                .get(name)
                .ok_or_else(|| format!("font asset {name} absent"))?;
            let (font, sheet) = LabelFont::decode(
                ui.get(&format!("font/{id}"))?,
                visual.get(&format!("sprite/{id}"))?,
            )?;
            atlases.push(Atlas::new(&sheet));
            fonts.push(font);
        }
        fn array<T>(v: Vec<T>) -> Result<[T; 3], String> {
            v.try_into().map_err(|_| "three label fonts".to_owned())
        }
        Ok((array(fonts)?, array(atlases)?))
    }

    fn flush(&self, atlas: &Atlas, vertices: &mut Vec<Vertex>) {
        for quads in vertices.chunks(4 * 4096) {
            let indices = (0..quads.len() as u16 / 4)
                .flat_map(|q| [0, 1, 2, 0, 2, 3].map(|i| q * 4 + i))
                .collect();
            draw_mesh(&Mesh {
                vertices: quads.to_vec(),
                indices,
                texture: Some(atlas.texture.clone()),
            });
        }
        vertices.clear();
    }

    /// Lays out and draws the labels of `list`; returns the number of raster
    /// calls the original would have made. The first font failure is
    /// reported once, after which labels stay off.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        ui: &Pack,
        visual: &Pack,
        list: &MapElementList,
        view: View,
        size: [i32; 2],
        origin: [i32; 2],
        fine: [i32; 2],
        at: [i32; 2],
        mask: (&Texture2D, [i32; 2]),
    ) -> Result<usize, String> {
        if matches!(self.fonts, Fonts::Unloaded) {
            match Self::load(ui, visual) {
                Ok(loaded) => self.fonts = Fonts::Loaded(Box::new(loaded)),
                Err(e) => {
                    self.fonts = Fonts::Failed;
                    return Err(format!("labels: {e}"));
                }
            }
        }
        let Fonts::Loaded(loaded) = &self.fonts else {
            return Ok(0);
        };
        let (fonts, atlases) = &**loaded;
        let draws = labels::layout(list, fonts, view, size, origin, fine);
        if draws.is_empty() {
            return Ok(0);
        }
        self.material.set_texture("MaskTexture", mask.0.clone());
        self.material.set_uniform(
            "MaskBounds",
            vec4(
                at[0] as f32,
                at[1] as f32,
                mask.1[0] as f32,
                mask.1[1] as f32,
            ),
        );
        gl_use_material(&self.material);
        let rgb = |color: i32, alpha: i32| {
            let mut c = Color::from_hex(color as u32 & 0xffffff);
            c.a = alpha.clamp(0, 256) as f32 / 256.0;
            c
        };
        let mut vertices = Vec::new();
        let mut current = 0;
        for draw in &draws {
            match *draw {
                Draw::Glyph {
                    font,
                    glyph,
                    at: [x, y],
                    size: [w, h],
                    color,
                    alpha,
                } => {
                    if w <= 0 || h <= 0 {
                        continue;
                    }
                    if font != current {
                        self.flush(&atlases[current], &mut vertices);
                        current = font;
                    }
                    let atlas = &atlases[font];
                    let u = (usize::from(glyph) % 16 * atlas.cell[0]) as f32;
                    let v = (usize::from(glyph) / 16 * atlas.cell[1]) as f32;
                    let (x, y) = ((at[0] + x) as f32, (at[1] + y) as f32);
                    let (w, h) = (w as f32, h as f32);
                    let color = rgb(color, alpha);
                    for [dx, dy] in [[0., 0.], [w, 0.], [w, h], [0., h]] {
                        vertices.push(Vertex::new(
                            x + dx,
                            y + dy,
                            0.,
                            (u + dx) / atlas.size[0],
                            (v + dy) / atlas.size[1],
                            color,
                        ));
                    }
                }
                Draw::Line {
                    at: [x, y],
                    width,
                    color,
                } => {
                    // GlRaster.drawHorizontalLine is clipped by the component
                    // rectangle only, which the caller's scissor supplies.
                    self.flush(&atlases[current], &mut vertices);
                    gl_use_default_material();
                    draw_rectangle(
                        (at[0] + x) as f32,
                        (at[1] + y) as f32,
                        width as f32,
                        1.,
                        rgb(color, 256),
                    );
                    gl_use_material(&self.material);
                }
            }
        }
        self.flush(&atlases[current], &mut vertices);
        gl_use_default_material();
        Ok(draws.len())
    }
}

const VERTEX: &str = r#"#version 100
attribute vec3 position;attribute vec2 texcoord;attribute vec4 color0;
uniform mat4 Model;uniform mat4 Projection;
varying highp vec2 uv;varying highp vec2 screen_position;varying lowp vec4 color;
void main(){gl_Position=Projection*Model*vec4(position,1.0);uv=texcoord;screen_position=position.xy;color=color0/255.0;}
"#;
const FRAGMENT: &str = r#"#version 100
precision highp float;varying highp vec2 uv;varying highp vec2 screen_position;varying lowp vec4 color;
uniform sampler2D Texture;uniform sampler2D MaskTexture;uniform vec4 MaskBounds;
void main(){float glyph=texture2D(Texture,uv).a;float mask=texture2D(MaskTexture,(screen_position-MaskBounds.xy)/MaskBounds.zw).a;gl_FragColor=vec4(color.rgb,color.a*glyph*mask);}
"#;
