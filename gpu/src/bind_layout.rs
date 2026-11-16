use wgpu::{BindGroupLayoutEntry, BindingType};

/// A reflected buffer binding with a dynamic offset: the same binding number,
/// visibility, buffer type and minimum size, with `has_dynamic_offset: true`.
///
/// WGSL has no syntax for a dynamic offset, so `exact-gpu-reflect` generates
/// every buffer entry without one. That is a choice the Rust makes when it
/// binds. A surface that binds one buffer at several offsets, one per pass,
/// builds its layout from the reflected entry instead of writing it out
/// again, so the reflected binding number, visibility and minimum size carry
/// through:
///
/// ```ignore
/// const STEP: wgpu::BindGroupLayoutEntry = exact_gpu::dynamic_offset(shader::S);
/// let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
///     label: Some("step"),
///     entries: &[STEP, shader::STATE],
/// });
/// ```
///
/// # Panics
///
/// Panics if `entry` is not a buffer binding. In a `const` initializer this
/// is a compile error.
pub const fn dynamic_offset(entry: BindGroupLayoutEntry) -> BindGroupLayoutEntry {
    match entry.ty {
        BindingType::Buffer {
            ty,
            min_binding_size,
            ..
        } => BindGroupLayoutEntry {
            ty: BindingType::Buffer {
                ty,
                has_dynamic_offset: true,
                min_binding_size,
            },
            ..entry
        },
        _ => panic!("dynamic_offset: only a buffer binding takes a dynamic offset"),
    }
}

#[cfg(test)]
mod tests {
    use super::dynamic_offset;
    use wgpu::{BindGroupLayoutEntry, BindingType, BufferBindingType, ShaderStages};

    // As `exact-gpu-reflect` generates a uniform binding.
    const S: BindGroupLayoutEntry = BindGroupLayoutEntry {
        binding: 3,
        visibility: ShaderStages::FRAGMENT.union(ShaderStages::COMPUTE),
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: ::core::num::NonZeroU64::new(48),
        },
        count: None,
    };

    #[test]
    fn a_uniform_keeps_everything_but_the_offset_flag() {
        const D: BindGroupLayoutEntry = dynamic_offset(S);
        assert_eq!(
            D,
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::FRAGMENT.union(ShaderStages::COMPUTE),
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: ::core::num::NonZeroU64::new(48),
                },
                count: None,
            }
        );
    }

    #[test]
    fn a_storage_buffer_keeps_its_access() {
        let read = BindGroupLayoutEntry {
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: ::core::num::NonZeroU64::new(4),
            },
            ..S
        };
        assert_eq!(
            dynamic_offset(read).ty,
            BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: true,
                min_binding_size: ::core::num::NonZeroU64::new(4),
            }
        );
    }

    #[test]
    #[should_panic(expected = "only a buffer binding takes a dynamic offset")]
    fn a_sampler_is_refused() {
        dynamic_offset(BindGroupLayoutEntry {
            ty: BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            ..S
        });
    }
}
