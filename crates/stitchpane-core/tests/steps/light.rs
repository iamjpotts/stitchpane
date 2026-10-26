//! Steps for light adjustments (LIGHT-001 to LIGHT-006) and dodge and burn (DODGE-001 to
//! DODGE-004).

use cucumber::{given, then, when};
use stitchpane_core::editor::Command;
use stitchpane_core::model::{ItemId, Light, StrokeKind, ToneRange};

use super::World;

/// Luminance-like brightness of a pixel, for lighter and darker checks.
fn brightness(p: [u8; 4]) -> f64 {
    0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2])
}

fn rendered_at(world: &mut World, x: u32, y: u32) -> [u8; 4] {
    world.editor.render_canvas().get_pixel(x, y).0
}

fn remembered_at(world: &World, x: u32, y: u32) -> [u8; 4] {
    world
        .remembered_canvas
        .as_ref()
        .expect("a remembered canvas")
        .get_pixel(x, y)
        .0
}

#[when(expr = "I start adjusting the light of image {int}")]
fn start_light(world: &mut World, id: u32) {
    assert!(world.editor.execute(Command::AdjustLight(ItemId(id))));
}

#[when(expr = "I set the light {word} to {float}")]
fn set_light(world: &mut World, setting: String, value: f64) {
    let mut light = world.editor.light_tool().expect("light tool active").light;
    match setting.as_str() {
        "exposure" => light.exposure = value,
        "shadows" => light.shadows = value / 100.0,
        "highlights" => light.highlights = value / 100.0,
        "radius" => light.radius = value,
        "black" => light.black = value,
        "white" => light.white = value,
        "midtones" => light.midtones = value,
        other => panic!("no light setting {other}"),
    }
    world.editor.set_light(light);
}

fn light_setting(light: &Light, setting: &str) -> f64 {
    match setting {
        "exposure" => light.exposure,
        "shadows" => light.shadows * 100.0,
        "highlights" => light.highlights * 100.0,
        "radius" => light.radius,
        "black" => light.black,
        "white" => light.white,
        "midtones" => light.midtones,
        other => panic!("no light setting {other}"),
    }
}

#[when(expr = "I {word} the light")]
fn light_command(world: &mut World, action: String) {
    let command = match action.as_str() {
        "apply" => Command::ApplyLight,
        "cancel" => Command::CancelLight,
        "reset" => Command::ResetLight,
        other => panic!("no light action {other}"),
    };
    assert!(world.editor.execute(command));
}

#[then(expr = "the light tool is active on image {int}")]
fn light_active(world: &mut World, id: u32) {
    assert_eq!(world.editor.light_tool().map(|t| t.item), Some(ItemId(id)));
}

#[then("the light tool is not active")]
fn light_inactive(world: &mut World) {
    assert_eq!(world.editor.light_tool(), None);
}

#[then(expr = "the light tool shows {word} {float}")]
fn light_shows(world: &mut World, setting: String, value: f64) {
    let light = world.editor.light_tool().expect("light tool active").light;
    let shown = light_setting(&light, &setting);
    assert!((shown - value).abs() < 1e-9, "{setting} is {shown}");
}

#[then(expr = "the light setting is refused with a message containing {string}")]
fn refused(world: &mut World, text: String) {
    let message = world.editor.take_message().expect("a message");
    assert!(message.contains(&text), "{message}");
}

#[given("I remember the rendered canvas")]
#[when("I remember the rendered canvas")]
fn remember_canvas(world: &mut World) {
    world.remembered_canvas = Some(world.editor.render_canvas());
}

#[then(regex = r"^the rendered canvas at (\d+), (\d+) is (lighter|darker) than remembered$")]
fn compared_to_remembered(world: &mut World, x: u32, y: u32, direction: String) {
    let before = brightness(remembered_at(world, x, y));
    let now = brightness(rendered_at(world, x, y));
    match direction.as_str() {
        "lighter" => assert!(now > before + 5.0, "{now} is not lighter than {before}"),
        _ => assert!(now < before - 5.0, "{now} is not darker than {before}"),
    }
}

#[then(expr = "the rendered canvas at {int}, {int} is as remembered")]
fn same_as_remembered(world: &mut World, x: u32, y: u32) {
    let before = remembered_at(world, x, y);
    let now = rendered_at(world, x, y);
    assert!(super::close(now, before, 2), "{now:?} is not {before:?}");
}

#[when(expr = "I start dodging image {int}")]
fn start_dodge(world: &mut World, id: u32) {
    assert!(world.editor.execute(Command::DodgeBurn(ItemId(id))));
}

fn kind(name: &str) -> StrokeKind {
    StrokeKind::DODGE_BURN
        .into_iter()
        .find(|k| k.label().eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("no dodge and burn kind {name}"))
}

fn range(name: &str) -> ToneRange {
    ToneRange::ALL
        .into_iter()
        .find(|r| r.label().eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("no range {name}"))
}

#[when(expr = "I set the dodge brush to {word} {word} {int} pixels wide at {int}% strength")]
fn set_dodge(world: &mut World, k: String, r: String, size: f64, strength: f64) {
    world.editor.set_dodge_kind(kind(&k));
    world.editor.set_dodge_range(range(&r));
    world.editor.set_dodge_size(size);
    world.editor.set_dodge_strength(strength / 100.0);
}

#[then(expr = "the dodge brush is {word} {word} {int} pixels wide at {int}% strength")]
fn dodge_is(world: &mut World, k: String, r: String, size: f64, strength: f64) {
    let tool = world
        .editor
        .dodge_tool()
        .expect("dodge and burn tool active");
    assert_eq!((tool.kind, tool.range), (kind(&k), range(&r)), "{tool:?}");
    assert_eq!(tool.size, size);
    assert!((tool.strength * 100.0 - strength).abs() < 1e-9, "{tool:?}");
}

#[then(expr = "the dodge and burn tool is active on image {int}")]
fn dodge_active(world: &mut World, id: u32) {
    assert_eq!(world.editor.dodge_tool().map(|t| t.item), Some(ItemId(id)));
}

#[then("the dodge and burn tool is not active")]
fn dodge_inactive(world: &mut World) {
    assert_eq!(world.editor.dodge_tool(), None);
}
