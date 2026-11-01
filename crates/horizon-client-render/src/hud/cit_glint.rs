//! OptiFine CIT enchantment glints on GUI items (`type=enchantment`, see `crate::cit_glint` for
//! which layers a stack draws): every rule's texture in one texture array, each layer an extra
//! pass over the item's quads with its rule's blend. A layer's vertices carry its texture slot
//! (red) and its intensity (alpha) in their colour; its scrolled UV is worked out on the CPU, as
//! vanilla's glint's is. Without enchantment rules nothing here is made.

use super::HudVertex;
use horizon_optifine::cit::{Blend, Factor, Kind, Rule};
use std::path::Path;

/// Slots the shader's table holds; rules past it draw no layer.
pub const MAX_SLOTS: usize = 256;
/// The largest side the shared texture size takes: bigger glints are scaled down to it.
const MAX_SIDE: u32 = 1024;

/// One slot's constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    /// The rule's `r`, `g`, `b`, `a`.
    pub rgba: [f32; 4],
    /// Linear filtering (`blur`), else nearest.
    pub blur: bool,
    /// An index into `GlintTextures::blends`.
    pub blend: u8,
}

/// The enchantment rules' textures, resized to one size.
#[derive(Debug, Default)]
pub struct GlintTextures {
    pub width: u32,
    pub height: u32,
    /// One `width` x `height` RGBA image per slot, slot after slot.
    pub pixels: Vec<u8>,
    /// Per rule (an index into `CitRules::rules`): its slot; `None` for other kinds of rules and
    /// textures that did not load.
    pub slot_of: Vec<Option<u8>>,
    pub slots: Vec<Slot>,
    /// The blends the slots use, once each: the HUD makes a pipeline per entry.
    pub blends: Vec<Option<Blend>>,
}

impl GlintTextures {
    /// The textures of `rules` under the overlay resource root.
    pub fn load(root: &Path, rules: &[Rule]) -> Self {
        let images = rules
            .iter()
            .enumerate()
            .filter(|(_, r)| r.kind == Kind::Enchantment)
            .filter_map(|(i, r)| {
                let path = r.texture.as_deref()?;
                match super::decode(&root.join(path)) {
                    Ok(image) => Some((i, image)),
                    Err(e) => {
                        log::warn!("custom item textures: glint {path} left out: {e}");
                        None
                    }
                }
            })
            .take(MAX_SLOTS)
            .collect();
        Self::new(rules, images)
    }

    /// `images`: (rule, (width, height, RGBA)).
    fn new(rules: &[Rule], images: Vec<(usize, (u32, u32, Vec<u8>))>) -> Self {
        let side = |pick: fn(&(u32, u32, Vec<u8>)) -> u32| {
            images.iter().map(|(_, im)| pick(im).min(MAX_SIDE)).max()
        };
        let (Some(width), Some(height)) = (side(|im| im.0), side(|im| im.1)) else {
            return Self::default();
        };
        let mut out = Self {
            width,
            height,
            slot_of: vec![None; rules.len()],
            ..Self::default()
        };
        for (rule, (w, h, rgba)) in images {
            out.pixels.extend(resize(&rgba, w, h, width, height));
            let glint = &rules[rule].glint;
            let blend = match out.blends.iter().position(|b| *b == glint.blend) {
                Some(i) => i,
                None => {
                    out.blends.push(glint.blend);
                    out.blends.len() - 1
                }
            };
            out.slot_of[rule] = Some(out.slots.len() as u8);
            out.slots.push(Slot {
                rgba: glint.rgba,
                blur: glint.blur,
                blend: blend as u8,
            });
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

/// Nearest-neighbour scaling: the glint repeats over the item, so its scale does not show.
fn resize(rgba: &[u8], w: u32, h: u32, to_w: u32, to_h: u32) -> Vec<u8> {
    if (w, h) == (to_w, to_h) {
        return rgba.to_vec();
    }
    let mut out = Vec::with_capacity((to_w * to_h * 4) as usize);
    for y in 0..to_h {
        let sy = (u64::from(y) * u64::from(h) / u64::from(to_h)) as usize;
        for x in 0..to_w {
            let sx = (u64::from(x) * u64::from(w) / u64::from(to_w)) as usize;
            let at = (sy * w as usize + sx) * 4;
            out.extend_from_slice(&rgba[at..at + 4]);
        }
    }
    out
}

/// CIT Resewn's glint texturing for one layer: `translation(-x, y)`, `rotateZ(rotation)`, then
/// `scale`, where `x` and `y` run through vanilla's 110000 and 30000 periods of the clock
/// `millis * speed` (vanilla's own glint runs at `millis * glintSpeed * 8`). `rotation` is in
/// degrees, as OptiFine's doc has it.
pub fn layer_uv(model: [f32; 2], millis: f32, speed: f32, rotation: f32, scale: f32) -> [f32; 2] {
    let clock = f64::from(millis) * f64::from(speed);
    let x = (clock % 110_000.0 / 110_000.0) as f32;
    let y = (clock % 30_000.0 / 30_000.0) as f32;
    let (sin, cos) = rotation.to_radians().sin_cos();
    let (u, v) = (model[0] * scale, model[1] * scale);
    [u * cos - v * sin - x, u * sin + v * cos + y]
}

/// The colour of a layer's vertices: its slot in red, its intensity in alpha.
pub fn vertex_color(slot: u8, intensity: f32) -> u32 {
    let alpha = (intensity.clamp(0.0, 1.0) * 255.0).round() as u8;
    super::rgba(slot, 0, 0, alpha)
}

fn factor(f: Factor) -> wgpu::BlendFactor {
    use wgpu::BlendFactor as W;
    match f {
        Factor::Zero => W::Zero,
        Factor::One => W::One,
        Factor::SrcColor => W::Src,
        Factor::OneMinusSrcColor => W::OneMinusSrc,
        Factor::SrcAlpha => W::SrcAlpha,
        Factor::OneMinusSrcAlpha => W::OneMinusSrcAlpha,
        Factor::DstAlpha => W::DstAlpha,
        Factor::OneMinusDstAlpha => W::OneMinusDstAlpha,
        Factor::DstColor => W::Dst,
        Factor::OneMinusDstColor => W::OneMinusDst,
        Factor::SrcAlphaSaturate => W::SrcAlphaSaturated,
    }
}

/// `glBlendFuncSeparate` with the rule's factors; `replace` draws unblended.
pub fn blend_state(blend: Option<Blend>) -> wgpu::BlendState {
    let Some(b) = blend else {
        return wgpu::BlendState::REPLACE;
    };
    let component = |src, dst| wgpu::BlendComponent {
        src_factor: factor(src),
        dst_factor: factor(dst),
        operation: wgpu::BlendOperation::Add,
    };
    wgpu::BlendState {
        color: component(b.src, b.dst),
        alpha: component(b.src_alpha, b.dst_alpha),
    }
}

/// The GPU side: the texture array and slot table (bind group 1 of `fs_cit_glint`), and one
/// pipeline per blend.
pub(super) struct CitGlintPass {
    group: wgpu::BindGroup,
    pipelines: Vec<wgpu::RenderPipeline>,
}

impl CitGlintPass {
    /// `None` without textures. `hud` is the HUD's bind group layout (group 0).
    pub(super) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        hud: &wgpu::BindGroupLayout,
        textures: &GlintTextures,
    ) -> Option<Self> {
        use wgpu::util::DeviceExt;
        if textures.is_empty() {
            return None;
        }
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("CIT glints"),
                size: wgpu::Extent3d {
                    width: textures.width,
                    height: textures.height,
                    depth_or_array_layers: textures.slots.len() as u32,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &textures.pixels,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let sampler = |label, filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        };
        let linear = sampler("CIT glint blur", wgpu::FilterMode::Linear);
        let nearest = sampler("CIT glint", wgpu::FilterMode::Nearest);
        // hud.wgsl's `CitSlot`: rgba, then blur in x.
        let mut table = vec![[0.0f32; 8]; MAX_SLOTS];
        for (row, slot) in table.iter_mut().zip(&textures.slots) {
            row[..4].copy_from_slice(&slot.rgba);
            row[4] = if slot.blur { 1.0 } else { 0.0 };
        }
        let table = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CIT glint slots"),
            contents: bytemuck::cast_slice(&table),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let fragment = wgpu::ShaderStages::FRAGMENT;
        let filtering = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: fragment,
            ty,
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("CIT glint bindings"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                ),
                entry(1, filtering),
                entry(2, filtering),
                entry(
                    3,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
            ],
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CIT glint resources"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&linear),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&nearest),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: table.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Horizon HUD (CIT glints)"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../hud.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("CIT glint layout"),
            bind_group_layouts: &[Some(hud), Some(&layout)],
            ..Default::default()
        });
        let attributes =
            wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Unorm8x4,3=>Float32,4=>Float32x2];
        let pipelines = textures
            .blends
            .iter()
            .map(|&blend| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("CIT glint"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[Some(wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<HudVertex>() as u64,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &attributes,
                        })],
                    },
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs_cit_glint"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(blend_state(blend)),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
            })
            .collect();
        Some(Self { group, pipelines })
    }

    /// Draw `vertices` with blend `blend` (an index into `GlintTextures::blends`).
    pub(super) fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        blend: u8,
        vertices: std::ops::Range<u32>,
    ) {
        if let Some(pipeline) = self.pipelines.get(usize::from(blend)) {
            pass.set_pipeline(pipeline);
            pass.set_bind_group(1, &self.group, &[]);
            pass.draw(vertices, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_optifine::Properties;
    use horizon_optifine::cit::parse_blend;

    fn rule(name: &str, text: &str) -> Rule {
        let path = format!("assets/minecraft/optifine/cit/{name}.properties");
        let text = format!("texture=glint\n{text}");
        Rule::parse(&path, &Properties::parse_equals_first(&text), |_| true).unwrap()
    }

    #[test]
    fn slots_share_one_size_and_their_blends() {
        let rules = [
            rule("a", "type=enchantment\nr=0.5"),
            rule("item", "items=stick"),
            rule("b", "type=enchantment\nblend=multiply\nblur=false"),
            rule("c", "type=enchantment"),
        ];
        // A 1x1 red glint and a 2x2 one: the red texel fills 2x2.
        let red = vec![255, 0, 0, 255];
        let quad: Vec<u8> = (0..16).collect();
        let t = GlintTextures::new(
            &rules,
            vec![
                (0, (1, 1, red)),
                (2, (2, 2, quad.clone())),
                (3, (2, 2, quad.clone())),
            ],
        );
        assert_eq!((t.width, t.height), (2, 2));
        assert_eq!(t.slot_of, [Some(0), None, Some(1), Some(2)]);
        assert_eq!(&t.pixels[..16], [255, 0, 0, 255].repeat(4));
        assert_eq!(&t.pixels[16..32], &quad[..]);
        assert_eq!(
            t.blends,
            [
                parse_blend("add").unwrap(),
                parse_blend("multiply").unwrap()
            ]
        );
        let blends: Vec<_> = t.slots.iter().map(|s| s.blend).collect();
        assert_eq!(blends, [0, 1, 0]);
        assert_eq!(t.slots[0].rgba, [0.5, 1.0, 1.0, 1.0]);
        assert!(!t.slots[1].blur && t.slots[2].blur);
        assert!(GlintTextures::new(&rules, Vec::new()).is_empty());
    }

    #[test]
    fn a_still_layer_is_scaled_and_turned() {
        let close =
            |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5;
        // No speed: no scroll whatever the clock.
        assert!(close(
            layer_uv([0.25, 0.5], 12_345.0, 0.0, 0.0, 8.0),
            [2.0, 4.0]
        ));
        // A quarter turn takes (1, 0) to (0, 1).
        assert!(close(layer_uv([1.0, 0.0], 0.0, 0.0, 90.0, 1.0), [0.0, 1.0]));
        // 1500 ms at speed 2: 3000 / 110000 back, 3000 / 30000 up.
        assert!(close(
            layer_uv([0.0, 0.0], 1500.0, 2.0, 0.0, 8.0),
            [-3.0 / 110.0, 0.1]
        ));
    }

    #[test]
    fn named_blends_take_their_gl_factors() {
        let multiply = blend_state(parse_blend("multiply").unwrap());
        assert_eq!(multiply.color.src_factor, wgpu::BlendFactor::Dst);
        assert_eq!(
            multiply.color.dst_factor,
            wgpu::BlendFactor::OneMinusSrcAlpha
        );
        // The alpha factors default to GL_ZERO, GL_ONE: the target's alpha stays.
        assert_eq!(multiply.alpha.src_factor, wgpu::BlendFactor::Zero);
        assert_eq!(multiply.alpha.dst_factor, wgpu::BlendFactor::One);
        assert_eq!(
            blend_state(parse_blend("replace").unwrap()),
            wgpu::BlendState::REPLACE
        );
        assert_eq!(vertex_color(3, 0.5), super::super::rgba(3, 0, 0, 128));
    }
}
