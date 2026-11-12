//! A surface that asks for Display P3 gets an 8-bit target in Display P3 on
//! a real `CAMetalLayer`, with no EDR, and is told so, again after its device
//! is restored; one that doesn't keeps wgpu's default, sRGB, and HDR wins.

use crate::{fixture, wgpu, Frame, Module, Registry, Surface, SurfaceError, Value};
use std::ffi::c_void;
use std::sync::Mutex;

#[link(name = "QuartzCore", kind = "framework")]
extern "C" {}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    static kCGColorSpaceDisplayP3: *const c_void;
    static kCGColorSpaceSRGB: *const c_void;
    fn CGColorSpaceCopyName(space: *const c_void) -> *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFEqual(a: *const c_void, b: *const c_void) -> u8;
    fn CFRelease(cf: *const c_void);
}

/// CoreGraphics' opaque colour space, encoded as `colorspace` returns it.
#[repr(C)]
struct CGColorSpace {
    _private: [u8; 0],
}
// SAFETY: a CGColorSpaceRef is a pointer to this opaque struct.
unsafe impl objc2::encode::RefEncode for CGColorSpace {
    const ENCODING_REF: objc2::encode::Encoding =
        objc2::encode::Encoding::Pointer(&objc2::encode::Encoding::Struct("CGColorSpace", &[]));
}

/// What each surface was told, in order: `device_ready` (`None`) and
/// `display_p3_granted` (`Some`).
static TOLD: Mutex<Vec<(&'static str, Option<bool>)>> = Mutex::new(Vec::new());

/// Its name, whether it asks for Display P3, and whether for HDR.
struct Probe(&'static str, bool, bool);
impl Surface for Probe {
    fn display_p3(&self) -> bool {
        self.1
    }
    fn high_dynamic_range(&self) -> bool {
        self.2
    }
    fn display_p3_granted(&mut self, granted: bool) {
        TOLD.lock().unwrap().push((self.0, Some(granted)));
    }
    fn device_ready(&mut self, _: wgpu::Features) {
        TOLD.lock().unwrap().push((self.0, None));
    }
    fn bind(&mut self, _: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn render(
        &mut self,
        _: &Frame,
        _: &wgpu::Device,
        _: &wgpu::Queue,
        _: &mut wgpu::CommandEncoder,
        _: &wgpu::TextureView,
        _: wgpu::TextureFormat,
    ) -> bool {
        false
    }
}

static REGISTRY: Registry = Registry {
    surfaces: &[
        ("p3", 0, || Box::new(Probe("p3", true, false))),
        ("p3-attached", 0, || {
            Box::new(Probe("p3-attached", true, false))
        }),
        ("srgb", 0, || Box::new(Probe("srgb", false, false))),
        ("hdr-and-p3", 0, || {
            Box::new(Probe("hdr-and-p3", true, true))
        }),
    ],
    shaders: &[],
};

/// Whether `layer`'s `colorspace` is named `name`; `None` when it has none.
fn layer_space_is(layer: &objc2::runtime::AnyObject, name: *const c_void) -> Option<bool> {
    // SAFETY: CAMetalLayer's `colorspace` is a CGColorSpaceRef or nil, owned
    // by the layer; the name is a copy, released here.
    unsafe {
        let space: *const CGColorSpace = objc2::msg_send![layer, colorspace];
        if space.is_null() {
            return None;
        }
        let copied = CGColorSpaceCopyName(space.cast());
        let same = !copied.is_null() && CFEqual(copied, name) != 0;
        if !copied.is_null() {
            CFRelease(copied);
        }
        Some(same)
    }
}

#[test]
fn a_display_p3_surface_gets_a_p3_layer_without_edr() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut m = Module::new(&REGISTRY);
    m.set_gpu(gpu);
    m.set_seekable(true);
    let mut layers = Vec::new();
    let mut make = |m: &mut Module, name: &str, headless: bool| {
        // SAFETY: a new CAMetalLayer, kept past the module.
        let layer: objc2::rc::Retained<objc2::runtime::AnyObject> =
            unsafe { objc2::msg_send![objc2::class!(CAMetalLayer), new] };
        let ptr = objc2::rc::Retained::as_ptr(&layer) as *mut c_void;
        // SAFETY: the layer outlives the module.
        let target = unsafe {
            m.gpu()
                .unwrap()
                .instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(ptr))
        }
        .unwrap();
        let id = if headless {
            let id = m.create_headless(name).unwrap();
            assert!(m.attach(id, target, 8, 8));
            id
        } else {
            m.create(name, target, 8, 8).unwrap()
        };
        layers.push(layer);
        id
    };
    let p3 = make(&mut m, "p3", false);
    let attached = make(&mut m, "p3-attached", true);
    let srgb = make(&mut m, "srgb", false);
    let both = make(&mut m, "hdr-and-p3", false);

    // SAFETY: CoreGraphics' constants, valid for the process.
    let (p3_name, srgb_name) = unsafe { (kCGColorSpaceDisplayP3, kCGColorSpaceSRGB) };
    assert_eq!(layer_space_is(&layers[0], p3_name), Some(true), "created");
    assert_eq!(layer_space_is(&layers[1], p3_name), Some(true), "attached");
    assert_eq!(
        layer_space_is(&layers[2], srgb_name),
        Some(true),
        "sRGB (on macOS a named sRGB space; on iOS wgpu-hal leaves it nil)"
    );
    assert_eq!(
        layer_space_is(&layers[3], p3_name),
        Some(false),
        "HDR's extended sRGB wins"
    );
    for (i, what) in [(0, "created"), (1, "attached")] {
        // SAFETY: CAMetalLayer's property, on a layer alive above.
        let edr: bool = unsafe { objc2::msg_send![&*layers[i], wantsExtendedDynamicRangeContent] };
        assert!(!edr, "Display P3 is SDR, so no EDR ({what})");
    }
    let format = |id| m.instances[&id].config.as_ref().unwrap().format;
    for id in [p3, attached] {
        assert!(
            matches!(
                format(id),
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
            ),
            "an 8-bit target: {:?}",
            format(id)
        );
    }
    assert_eq!(format(both), wgpu::TextureFormat::Rgba16Float);
    let _ = srgb;

    let last = |told: &[(&str, Option<bool>)], name: &str| {
        told.iter().filter(|t| t.0 == name).last().and_then(|t| t.1)
    };
    let told = TOLD.lock().unwrap().clone();
    for (name, want) in [
        ("p3", true),
        ("p3-attached", true),
        ("srgb", false),
        ("hdr-and-p3", false),
    ] {
        assert_eq!(last(&told, name), Some(want), "{name}: {told:?}");
    }

    // A restored device (the same one, re-set) tells each surface again,
    // after `device_ready` and before it draws.
    let before = TOLD.lock().unwrap().len();
    let gpu = m.gpu().unwrap();
    let again = crate::Gpu {
        instance: gpu.instance.clone(),
        adapter: gpu.adapter.clone(),
        device: gpu.device.clone(),
        queue: gpu.queue.clone(),
    };
    m.set_gpu(again);
    let told = TOLD.lock().unwrap().clone();
    let after: Vec<_> = told[before..]
        .iter()
        .filter(|t| t.0 == "p3")
        .map(|t| t.1)
        .collect();
    assert_eq!(
        after,
        [None, Some(true)],
        "device_ready, then granted: {told:?}"
    );
    assert_eq!(layer_space_is(&layers[0], p3_name), Some(true), "still P3");
    drop(m);
    drop(layers);
}
