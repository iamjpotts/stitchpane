use cucumber::then;
use image::RgbaImage;
use stitchpane_core::model::{CanvasSize, Curve, ItemId, Orientation, TransitionKind};

use super::{World, close, color};

fn item(world: &World, id: u32) -> stitchpane_core::model::Item {
    *world
        .editor
        .display_state()
        .item(ItemId(id))
        .unwrap_or_else(|| panic!("no image {id}"))
}

#[then(expr = "the crop rectangle is {int}, {int}, {int}, {int}")]
fn crop_rect(world: &mut World, l: u32, t: u32, w: u32, h: u32) {
    let r = world.editor.crop_tool().expect("crop tool").pixel_rect();
    assert_eq!((r.left, r.top, r.width, r.height), (l, t, w, h));
}

#[then(expr = "the crop rectangle has an aspect ratio of {float}")]
fn crop_ratio(world: &mut World, ratio: f64) {
    let r = world.editor.crop_tool().expect("crop tool").rect;
    let actual = r.width / r.height;
    assert!((actual - ratio).abs() < 0.01, "{actual}");
}

#[then("the crop tool is not active")]
fn crop_inactive(world: &mut World) {
    assert!(world.editor.crop_tool().is_none());
}

#[then(expr = "the canvas is {int} by {int} pixels")]
fn canvas(world: &mut World, w: u32, h: u32) {
    assert_eq!(world.editor.display_state().canvas_size(), (w, h));
}

#[then(expr = "the canvas DPI is {float}")]
fn canvas_dpi(world: &mut World, dpi: f64) {
    assert_eq!(world.editor.display_state().canvas.dpi, Some(dpi));
}

#[then("the canvas follows the image")]
fn canvas_fit(world: &mut World) {
    assert_eq!(world.editor.display_state().canvas.size, CanvasSize::Fit);
}

#[then(expr = "the image is {int} by {int} pixels")]
fn the_image_size(world: &mut World, w: u32, h: u32) {
    let id = world.item().0;
    image_size(world, id, w, h);
}

#[then(expr = "image {int} is {int} by {int} pixels")]
fn image_size(world: &mut World, id: u32, w: u32, h: u32) {
    let i = item(world, id);
    assert_eq!((i.width, i.height), (w, h));
}

#[then(expr = "image {int} is at {int}, {int}")]
fn image_at(world: &mut World, id: u32, x: i32, y: i32) {
    let i = item(world, id);
    assert_eq!((i.x, i.y), (x, y));
}

#[then(expr = "image {int} is on top")]
fn on_top(world: &mut World, id: u32) {
    let state = world.editor.display_state();
    assert_eq!(state.items.last().map(|i| i.id), Some(ItemId(id)));
}

#[then(expr = "image {int} is rotated {float} degrees")]
fn rotated(world: &mut World, id: u32, degrees: f64) {
    let r = item(world, id).rotation;
    assert!((r - degrees).abs() < 1e-6, "{r}");
}

#[then(expr = "the image's resampling filter is {string}")]
fn filter(world: &mut World, name: String) {
    let id = world.item().0;
    assert_eq!(item(world, id).filter.label(), name);
}

#[then(expr = "image {int}'s feather width is {int}")]
fn feather_width(world: &mut World, id: u32, width: f64) {
    let f = item(world, id).feather.expect("feathered");
    assert!((f.width - width).abs() < 1e-6, "{}", f.width);
}

fn rendered(world: &mut World) -> RgbaImage {
    world.editor.render_canvas()
}

fn rendered_at(world: &mut World, x: u32, y: u32) -> [u8; 4] {
    let image = rendered(world);
    image.get_pixel(x, y).0
}

#[then(regex = r#"^the rendered canvas at (\d+), (\d+) is (roughly )?"([^"]+)"$"#)]
fn rendered_color(world: &mut World, x: u32, y: u32, roughly: String, name: String) {
    let pixel = rendered_at(world, x, y);
    let tolerance = if roughly.is_empty() { 3 } else { 48 };
    let expected = color(&name);
    let ok = if name == "transparent" {
        pixel[3] <= tolerance
    } else {
        close(pixel, expected, tolerance)
    };
    assert!(ok, "{pixel:?} is not {name}");
}

#[then(expr = "the rendered canvas at {int}, {int} is roughly {int}, {int}, {int}")]
fn rendered_rgb(world: &mut World, x: u32, y: u32, r: u8, g: u8, b: u8) {
    let pixel = rendered_at(world, x, y);
    assert!(close(pixel, [r, g, b, 255], 3), "{pixel:?}");
}

#[then(expr = "the rendered canvas at {int}, {int} is a mix of red and blue")]
fn mix(world: &mut World, x: u32, y: u32) {
    let [r, g, b, a] = rendered_at(world, x, y);
    assert!((64..=192).contains(&r) && (64..=192).contains(&b) && g < 8 && a == 255);
}

#[then(expr = "the rendered canvas at {int}, {int} is nearly transparent")]
fn nearly_transparent(world: &mut World, x: u32, y: u32) {
    let pixel = rendered_at(world, x, y);
    assert!(pixel[3] < 40, "{pixel:?}");
}

#[then(expr = "the rendered canvas at {int}, {int} is gray")]
fn gray(world: &mut World, x: u32, y: u32) {
    let [r, g, b, a] = rendered_at(world, x, y);
    assert!(r == g && g == b && (100..=200).contains(&r) && a == 255);
}

#[then(expr = "the rendered canvas alpha at {int}, {int} is about {int}")]
fn alpha_about(world: &mut World, x: u32, y: u32, alpha: u8) {
    let pixel = rendered_at(world, x, y);
    assert!(pixel[3].abs_diff(alpha) <= 12, "{pixel:?}");
}

fn compare_rotated(world: &mut World, tolerance: u8) {
    compare_turned(world, tolerance, true);
}

fn compare_turned(world: &mut World, tolerance: u8, clockwise: bool) {
    let source = world.images.last().expect("source image").clone();
    let raw = image::open(&source).expect("open source").to_rgba8();
    let expected = if clockwise {
        image::imageops::rotate90(&raw)
    } else {
        image::imageops::rotate270(&raw)
    };
    let actual = rendered(world);
    assert_eq!(actual.dimensions(), expected.dimensions());
    for (a, e) in actual.pixels().zip(expected.pixels()) {
        assert!(close(a.0, e.0, tolerance), "{a:?} vs {e:?}");
    }
}

#[then(
    regex = r"^the rendered canvas is exactly the source rotated a quarter turn (clockwise|counterclockwise)$"
)]
fn exactly_rotated(world: &mut World, direction: String) {
    compare_turned(world, 0, direction == "clockwise");
}

#[then("the rendered canvas is roughly the source rotated a quarter turn clockwise")]
fn roughly_rotated(world: &mut World) {
    compare_rotated(world, 4);
}

#[then(regex = r"^there (?:is|are) (\d+) transitions?$")]
fn transitions(world: &mut World, count: usize) {
    assert_eq!(world.editor.display_state().transitions.len(), count);
}

fn seam(world: &World) -> stitchpane_core::model::SeamGeometry {
    let state = world.editor.display_state();
    let t = state.transitions[0];
    state
        .transition_geometry(&t)
        .expect("transition in effect")
        .seam
        .expect("a gradient seam")
}

#[then(expr = "the seam is at {int} with a band {int} wide")]
fn seam_at(world: &mut World, position: f64, width: f64) {
    let s = seam(world);
    assert_eq!(s.orientation, Orientation::Horizontal);
    assert!((s.position - position).abs() < 1e-6, "{}", s.position);
    assert!((s.width - width).abs() < 1e-6, "{}", s.width);
}

#[then(expr = "the seam is vertical at {int} with a band {int} wide")]
fn seam_vertical(world: &mut World, position: f64, width: f64) {
    let s = seam(world);
    assert_eq!(s.orientation, Orientation::Vertical);
    assert!((s.position - position).abs() < 1e-6, "{}", s.position);
    assert!((s.width - width).abs() < 1e-6, "{}", s.width);
}

#[then("the seam curve is smooth")]
fn seam_smooth(world: &mut World) {
    assert_eq!(seam(world).curve, Curve::Smooth);
}

#[then(expr = "the transition is a crossfade at {int}%")]
fn crossfade_at(world: &mut World, percent: f64) {
    let t = world.editor.display_state().transitions[0];
    assert_eq!(
        t.kind,
        TransitionKind::Crossfade {
            ratio: percent / 100.0
        }
    );
}

#[then("the transition has no effect")]
fn no_effect(world: &mut World) {
    let state = world.editor.display_state();
    assert!(state.transition_geometry(&state.transitions[0]).is_none());
}

#[then(expr = "image {int} is selected")]
fn selected(world: &mut World, id: u32) {
    assert_eq!(world.editor.selection().item(), Some(ItemId(id)));
}

#[then(expr = "image {int} is shown whole and on top while cropping")]
fn shown_whole(world: &mut World, id: u32) {
    let state = world.editor.display_state();
    let item = state.items.last().expect("items");
    assert_eq!(item.id, ItemId(id));
    assert_eq!(
        (
            item.crop.left,
            item.crop.top,
            item.crop.width,
            item.crop.height
        ),
        (0, 0, item.source_width, item.source_height)
    );
}

#[then(expr = "the rendered canvas at {int}, {int} is no longer {string}")]
fn rendered_changed(world: &mut World, x: u32, y: u32, name: String) {
    let pixel = rendered_at(world, x, y);
    assert!(!close(pixel, color(&name), 12), "{pixel:?} is still {name}");
}
