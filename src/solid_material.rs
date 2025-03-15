use three_d::*;

///
/// A material that renders a [Geometry] in a color defined by multiplying a color with an optional texture and optional per vertex colors.
/// This material is not affected by lights.
///
#[derive(Clone, Default)]
pub struct SolidMaterial {
    /// Base surface color.
    pub color: Srgba,
    /// Render states.
    pub render_states: RenderStates,
    /// Whether this material should be treated as a transparent material (An object needs to be rendered differently depending on whether it is transparent or opaque).
    pub is_transparent: bool,
}

impl SolidMaterial {
    ///
    /// Constructs a new color material from a [CpuMaterial].
    /// Tries to infer whether this material is transparent or opaque from the alpha value of the albedo color and the alpha values in the albedo texture.
    /// Since this is not always correct, it is preferred to use [ColorMaterial::new_opaque] or [ColorMaterial::new_transparent].
    ///
    pub fn new(context: &Context, cpu_material: &CpuMaterial) -> Self {
        Self::new_opaque(context, cpu_material)
    }

    /// Constructs a new opaque color material from a [CpuMaterial].
    pub fn new_opaque(context: &Context, cpu_material: &CpuMaterial) -> Self {
        Self {
            color: cpu_material.albedo,
            is_transparent: false,
            render_states: RenderStates::default(),
        }
    }

    /// Constructs a new transparent color material from a [CpuMaterial].
    pub fn new_transparent(context: &Context, cpu_material: &CpuMaterial) -> Self {
        Self {
            color: cpu_material.albedo,
            is_transparent: true,
            render_states: RenderStates {
                write_mask: WriteMask::COLOR,
                blend: Blend::TRANSPARENCY,
                ..Default::default()
            },
        }
    }

    /// Creates a color material from a [PhysicalMaterial].
    pub fn from_physical_material(physical_material: &PhysicalMaterial) -> Self {
        Self {
            color: physical_material.albedo,
            render_states: physical_material.render_states,
            is_transparent: physical_material.is_transparent,
        }
    }
}

impl FromCpuMaterial for SolidMaterial {
    fn from_cpu_material(context: &Context, cpu_material: &CpuMaterial) -> Self {
        Self::new(context, cpu_material)
    }
}

impl Material for SolidMaterial {
    fn id(&self) -> EffectMaterialId {
        EffectMaterialId(0x0000)
    }

    fn fragment_shader_source(&self, _lights: &[&dyn Light]) -> String {
        let mut shader = String::new();
        shader.push_str(ColorMapping::fragment_shader_source());
        shader.push_str(include_str!("solid_material_shader.frag"));
        shader
    }

    fn use_uniforms(&self, program: &Program, viewer: &dyn Viewer, _lights: &[&dyn Light]) {
        program.use_uniform("surfaceColor", self.color.to_linear_srgb());
        program.use_uniform_if_required("cameraPosition", viewer.position());
    }

    fn render_states(&self) -> RenderStates {
        self.render_states
    }

    fn material_type(&self) -> MaterialType {
        if self.is_transparent {
            MaterialType::Transparent
        } else {
            MaterialType::Opaque
        }
    }
}