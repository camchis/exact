use crate::{wgpu, Surface};

pub(crate) fn configure(
    surface: &dyn Surface,
    adapter: &wgpu::Adapter,
    caps: &wgpu::SurfaceCapabilities,
    config: &mut wgpu::SurfaceConfiguration,
) -> (bool, bool) {
    let views = adapter
        .get_downlevel_capabilities()
        .flags
        .contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS);
    let hdr = surface.high_dynamic_range()
        && caps
            .color_spaces(wgpu::TextureFormat::Rgba16Float)
            .contains(wgpu::SurfaceColorSpaces::EXTENDED_SRGB);
    if hdr {
        config.format = wgpu::TextureFormat::Rgba16Float;
        config.color_space = wgpu::SurfaceColorSpace::ExtendedSrgb;
        config.view_formats.clear();
    } else {
        let native_srgb = adapter.get_info().backend == wgpu::Backend::Vulkan;
        let (format, view) = sdr_format(
            surface.srgb_target(),
            views,
            native_srgb,
            &caps.formats,
            config.format,
        );
        config.format = format;
        config.view_formats = view.into_iter().collect();
        config.color_space = sdr_color_space(surface.display_p3(), caps.color_spaces(format));
    }
    config.alpha_mode = alpha_mode(surface.opaque(), &caps.alpha_modes);
    let p3 = config.color_space == wgpu::SurfaceColorSpace::DisplayP3;
    (hdr, p3)
}

/// Display P3 where the surface asks and the target offers it for the
/// format; else wgpu's default, sRGB for these 8-bit formats.
fn sdr_color_space(p3: bool, offered: wgpu::SurfaceColorSpaces) -> wgpu::SurfaceColorSpace {
    if p3 && offered.contains(wgpu::SurfaceColorSpaces::DISPLAY_P3) {
        wgpu::SurfaceColorSpace::DisplayP3
    } else {
        wgpu::SurfaceColorSpace::Auto
    }
}

/// How the compositor takes a canvas's alpha: ignored when the surface
/// promises every pixel opaque, else blended where the target can, else
/// the window's own policy (`Inherit`: Android's `Opaque`/`Inherit` pair),
/// else whatever the target offers. Never `Auto`: wgpu-core resolves it to
/// `Opaque` before `Inherit`, which would ignore a translucent canvas's
/// alpha, and wgpu-hal's Vulkan backend cannot map it at all.
pub(crate) fn alpha_mode(
    opaque: bool,
    modes: &[wgpu::CompositeAlphaMode],
) -> wgpu::CompositeAlphaMode {
    use wgpu::CompositeAlphaMode::{Inherit, Opaque, PostMultiplied, PreMultiplied};
    let blended = [PreMultiplied, PostMultiplied, Inherit];
    [Opaque]
        .iter()
        .filter(|_| opaque)
        .chain(&blended)
        .chain(modes.first())
        .copied()
        .find(|m| modes.contains(m))
        .unwrap_or(Inherit)
}

fn sdr_format(
    srgb: bool,
    views: bool,
    native_srgb: bool,
    formats: &[wgpu::TextureFormat],
    fallback: wgpu::TextureFormat,
) -> (wgpu::TextureFormat, Option<wgpu::TextureFormat>) {
    // Ordinary canvases encode their own output, as on the web. A surface
    // requesting linear-light rendering can instead use an sRGB target.
    let format = formats
        .iter()
        .copied()
        .find(|f| !f.is_srgb())
        .unwrap_or(fallback);
    let encoded = format.add_srgb_suffix();
    if srgb && encoded != format {
        if views {
            return (format, Some(encoded));
        }
        // Vulkan presents encoded bytes. GLES/WebGL encodes at present,
        // so a native sRGB attachment there would encode twice.
        if native_srgb && formats.contains(&encoded) {
            return (encoded, None);
        }
    }
    (format, None)
}

impl crate::Module {
    /// Instance policy before attachment: bit 0 promises alpha one; bit 1 requests SDR.
    pub fn presentation_flags(&self, id: u32) -> u32 {
        self.instances.get(&id).map_or(0, |i| {
            u32::from(i.surface.opaque()) | (u32::from(!i.surface.high_dynamic_range()) << 1)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_modes_are_ones_the_target_lists() {
        use wgpu::CompositeAlphaMode::{Inherit, Opaque, PostMultiplied, PreMultiplied};
        // Android's windows; Metal's layers; a Vulkan display with all.
        let android = [Opaque, Inherit];
        let metal = [Opaque, PostMultiplied];
        let all = [Opaque, PreMultiplied, PostMultiplied, Inherit];
        assert_eq!(alpha_mode(true, &android), Opaque);
        assert_eq!(alpha_mode(false, &android), Inherit);
        assert_eq!(alpha_mode(true, &metal), Opaque);
        assert_eq!(alpha_mode(false, &metal), PostMultiplied);
        assert_eq!(alpha_mode(false, &all), PreMultiplied);
        assert_eq!(alpha_mode(true, &[Inherit, PreMultiplied]), PreMultiplied);
        assert_eq!(alpha_mode(false, &[Opaque]), Opaque);
    }

    #[test]
    fn display_p3_only_where_asked_and_offered() {
        use wgpu::SurfaceColorSpace::{Auto, DisplayP3};
        use wgpu::SurfaceColorSpaces as S;
        assert_eq!(sdr_color_space(true, S::SRGB | S::DISPLAY_P3), DisplayP3);
        assert_eq!(sdr_color_space(false, S::SRGB | S::DISPLAY_P3), Auto);
        assert_eq!(sdr_color_space(true, S::SRGB), Auto, "not offered: sRGB");
        assert_eq!(sdr_color_space(true, S::empty()), Auto);
    }

    #[test]
    fn srgb_targets_respect_swapchain_format_support() {
        use wgpu::TextureFormat::{Bgra8Unorm as Linear, Bgra8UnormSrgb as Srgb};
        let both = [Linear, Srgb];
        assert_eq!(
            sdr_format(true, false, false, &both, Linear),
            (Linear, None)
        );
        assert_eq!(
            sdr_format(true, true, false, &both, Linear),
            (Linear, Some(Srgb))
        );
        assert_eq!(sdr_format(true, false, true, &both, Linear), (Srgb, None));
        assert_eq!(
            sdr_format(true, false, true, &[Linear], Linear),
            (Linear, None)
        );
        assert_eq!(sdr_format(false, true, false, &both, Srgb), (Linear, None));
        assert_eq!(sdr_format(true, false, true, &[Srgb], Srgb), (Srgb, None));
    }
}
