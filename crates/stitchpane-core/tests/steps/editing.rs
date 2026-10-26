use cucumber::{given, then, when};
use stitchpane_core::editor::{
    AspectChoice, Button, Command, Edge, Key, KeyOutcome, Modifiers, PrintUnit, Selection,
    proportional,
};
use stitchpane_core::geometry::{PixelRect, Point};
use stitchpane_core::model::{
    Curve, Feather, Filter, ItemId, Orientation, StrokeKind, TransitionKind,
};

use super::World;

fn mods(text: &str) -> Modifiers {
    Modifiers {
        shift: text.contains("Shift"),
        ctrl: false,
        alt: text.contains("Alt"),
    }
}

#[given(expr = "the view is at 100% with the canvas origin on screen at {int}, {int}")]
fn view_at(world: &mut World, x: f64, y: f64) {
    world.editor.set_view(1.0, Point::new(-x, -y));
}

#[given("snapping is off")]
fn snapping_off(world: &mut World) {
    if world.editor.snapping() {
        world.editor.execute(Command::ToggleSnapping);
    }
}

#[given(expr = "I select image {int}")]
#[when(expr = "I select image {int}")]
fn select_image(world: &mut World, id: u32) {
    world.editor.select(Selection::Item(ItemId(id)));
    world.item = Some(ItemId(id));
}

#[when("I select the transition")]
fn select_transition(world: &mut World) {
    let id = world.editor.project().state().transitions[0].id;
    world.editor.select(Selection::Transition(id));
}

fn drag(world: &mut World, points: &[(f64, f64)], modifiers: Modifiers, release: bool) {
    let screen: Vec<Point> = points.iter().map(|&(x, y)| world.screen(x, y)).collect();
    world
        .editor
        .pointer_down(screen[0], Button::Primary, modifiers);
    for &p in &screen[1..] {
        world.editor.pointer_move(p, modifiers);
    }
    if release {
        world
            .editor
            .pointer_up(*screen.last().expect("points"), modifiers);
    }
}

#[when(
    regex = r"^I drag from (-?[\d.]+), (-?[\d.]+) to (-?[\d.]+), (-?[\d.]+)( with Shift| holding Alt)?$"
)]
fn drag_to(world: &mut World, x0: f64, y0: f64, x1: f64, y1: f64, modifier: String) {
    drag(world, &[(x0, y0), (x1, y1)], mods(&modifier), true);
}

#[when(
    regex = r"^I drag from (-?[\d.]+), (-?[\d.]+) through (-?[\d.]+), (-?[\d.]+) and (-?[\d.]+), (-?[\d.]+) to (-?[\d.]+), (-?[\d.]+)$"
)]
#[allow(clippy::too_many_arguments)]
fn drag_through(
    world: &mut World,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x3: f64,
    y3: f64,
) {
    drag(
        world,
        &[(x0, y0), (x1, y1), (x2, y2), (x3, y3)],
        Modifiers::NONE,
        true,
    );
}

#[when(expr = "I move the pointer from {int}, {int} to {int}, {int} without releasing")]
fn drag_hold(world: &mut World, x0: f64, y0: f64, x1: f64, y1: f64) {
    drag(world, &[(x0, y0), (x1, y1)], Modifiers::NONE, false);
}

#[when(expr = "I release the pointer at {int}, {int}")]
fn release(world: &mut World, x: f64, y: f64) {
    let p = world.screen(x, y);
    world.editor.pointer_up(p, Modifiers::NONE);
}

fn parse_key(text: &str) -> (Key, Modifiers) {
    let mut modifiers = Modifiers::NONE;
    let mut key = text;
    loop {
        if let Some(rest) = key.strip_prefix("Ctrl+") {
            modifiers.ctrl = true;
            key = rest;
        } else if let Some(rest) = key.strip_prefix("Shift+") {
            modifiers.shift = true;
            key = rest;
        } else if let Some(rest) = key.strip_prefix("Alt+") {
            modifiers.alt = true;
            key = rest;
        } else {
            break;
        }
    }
    let key = match key {
        "Left" => Key::Left,
        "Right" => Key::Right,
        "Up" => Key::Up,
        "Down" => Key::Down,
        "Enter" => Key::Enter,
        "Escape" => Key::Escape,
        "Delete" => Key::Delete,
        "Backspace" => Key::Backspace,
        "Plus" => Key::Plus,
        "Minus" => Key::Minus,
        "[" => Key::BracketLeft,
        "]" => Key::BracketRight,
        other => Key::Char(other.chars().next().expect("key").to_ascii_lowercase()),
    };
    (key, modifiers)
}

fn press(world: &mut World, text: &str) {
    let (key, modifiers) = parse_key(text);
    match world.editor.key_press(key, modifiers) {
        KeyOutcome::Command(command) => world.ui_request = Some(command),
        KeyOutcome::Handled => {}
        KeyOutcome::Unhandled => panic!("{text} was not handled"),
    }
}

#[when(expr = "I press {string}")]
fn press_key(world: &mut World, text: String) {
    press(world, &text);
}

#[when(expr = "I press {string} {int} times")]
fn press_times(world: &mut World, text: String, times: usize) {
    for _ in 0..times {
        press(world, &text);
    }
}

#[when("I start cropping the image")]
fn start_crop(world: &mut World) {
    let item = world.item();
    assert!(world.editor.execute(Command::Crop(item)));
}

#[when(expr = "I set the crop rectangle to {int}, {int}, {int}, {int}")]
fn set_crop(world: &mut World, l: u32, t: u32, w: u32, h: u32) {
    world.editor.set_crop_rect(PixelRect::new(l, t, w, h));
}

#[when(expr = "I choose the crop aspect ratio {string}")]
fn crop_aspect(world: &mut World, label: String) {
    let aspect = AspectChoice::PRESETS
        .into_iter()
        .find(|a| a.label() == label)
        .unwrap_or_else(|| {
            let (w, h) = label.split_once(':').expect("ratio");
            AspectChoice::Custom(w.parse().expect("w"), h.parse().expect("h"))
        });
    world.editor.set_crop_aspect(aspect);
}

/// A canvas point where `id` is the topmost item.
fn point_on(world: &World, id: u32) -> Point {
    let state = world.editor.display_state();
    let item = state.item(ItemId(id)).expect("image");
    let b = item.bounds();
    for fy in 1..10 {
        for fx in 1..10 {
            let p = Point::new(
                b.x + b.width * f64::from(fx) / 10.0,
                b.y + b.height * f64::from(fy) / 10.0,
            );
            let top = state.items.iter().rev().find(|i| i.contains(p));
            if top.map(|i| i.id) == Some(ItemId(id)) {
                return p;
            }
        }
    }
    panic!("image {id} is hidden");
}

fn context_menu(world: &mut World, p: Point) {
    let screen = world.editor.view().to_screen(p);
    world.menu = world
        .editor
        .pointer_down(screen, Button::Secondary, Modifiers::NONE)
        .expect("context menu");
}

#[when(expr = "I open the context menu on image {int}")]
fn menu_on_image(world: &mut World, id: u32) {
    let p = point_on(world, id);
    context_menu(world, p);
}

#[when(expr = "I open the context menu at {int}, {int}")]
fn menu_at(world: &mut World, x: f64, y: f64) {
    context_menu(world, Point::new(x, y));
}

#[when(expr = "I choose {string}")]
fn choose(world: &mut World, label: String) {
    let command = world
        .menu
        .iter()
        .find(|e| e.label == label)
        .and_then(|e| e.command.clone())
        .unwrap_or_else(|| panic!("no menu entry {label:?} in {:?}", world.menu));
    if command.needs_ui() {
        world.ui_request = Some(command);
    } else {
        assert!(world.editor.execute(command), "{label} failed");
    }
}

#[then("the UI is asked for the transition properties dialog")]
fn asked_transition(world: &mut World) {
    assert!(matches!(
        world.ui_request,
        Some(Command::TransitionProperties(_))
    ));
}

#[then("the UI is asked for the feather dialog")]
fn asked_feather(world: &mut World) {
    assert!(matches!(world.ui_request, Some(Command::Feather(_))));
}

#[when(expr = "I resize the image to width {int} with the aspect ratio locked")]
fn resize_locked(world: &mut World, width: u32) {
    let item = world.item();
    let i = *world.editor.project().state().item(item).expect("image");
    let height = proportional(i.width, i.height, width);
    assert!(world.editor.resize_item(item, width, height, i.filter));
}

#[when(
    regex = r#"^I resize (?:the image|image (\d+)) to (\d+) by (\d+) pixels(?: using "([^"]+)")?$"#
)]
fn resize(world: &mut World, id: String, width: u32, height: u32, filter: String) {
    let item = id.parse().map(ItemId).unwrap_or_else(|_| world.item());
    let filter = Filter::ALL
        .into_iter()
        .find(|f| f.label() == filter)
        .unwrap_or_else(|| {
            world
                .editor
                .project()
                .state()
                .item(item)
                .expect("image")
                .filter
        });
    assert!(world.editor.resize_item(item, width, height, filter));
}

#[given(expr = "I set the DPI to {int}")]
#[when(expr = "I set the DPI to {int}")]
fn set_dpi(world: &mut World, dpi: f64) {
    assert!(world.editor.set_dpi(Some(dpi)));
}

#[when(expr = "I set the print size to {float} by {float} {word} at {int} DPI")]
fn print_size(world: &mut World, w: f64, h: f64, unit: String, dpi: f64) {
    let unit = if unit == "cm" {
        PrintUnit::Centimeters
    } else {
        PrintUnit::Inches
    };
    let item = world.item();
    assert!(
        world
            .editor
            .set_print_size(item, w, h, unit, dpi, Filter::Lanczos3)
    );
}

#[when(expr = "I set the canvas size to {int} by {int} pixels at {int} DPI")]
fn canvas_size(world: &mut World, w: u32, h: u32, dpi: f64) {
    assert!(world.editor.set_canvas_size(w, h, Some(dpi)));
}

#[when(expr = "I rotate image {int} to {float} degrees")]
fn rotate_to(world: &mut World, id: u32, degrees: f64) {
    assert!(world.editor.rotate_to(ItemId(id), degrees));
}

#[given(expr = "a horizontal gradient seam between image {int} and image {int}")]
fn seam_between(world: &mut World, a: u32, b: u32) {
    assert!(world.editor.execute(Command::AddSeam(
        [ItemId(a), ItemId(b)],
        Orientation::Horizontal
    )));
}

#[given(expr = "a crossfade between image {int} and image {int}")]
#[when(expr = "I add a crossfade between image {int} and image {int}")]
fn crossfade_between(world: &mut World, a: u32, b: u32) {
    assert!(
        world
            .editor
            .execute(Command::AddCrossfade([ItemId(a), ItemId(b)]))
    );
}

fn first_transition(world: &World) -> stitchpane_core::model::Transition {
    world.editor.display_state().transitions[0]
}

#[when("I set the seam to vertical, 30 wide, with a smooth curve")]
fn edit_seam(world: &mut World) {
    let t = first_transition(world);
    let TransitionKind::GradientSeam { offset, .. } = t.kind else {
        panic!("not a seam");
    };
    assert!(world.editor.edit_transition(
        t.id,
        TransitionKind::GradientSeam {
            orientation: Orientation::Vertical,
            offset,
            width: 30.0,
            curve: Curve::Smooth,
        }
    ));
}

#[when(expr = "I set the crossfade ratio to {int}%")]
fn crossfade_ratio(world: &mut World, percent: u32) {
    let t = first_transition(world);
    assert!(world.editor.edit_transition(
        t.id,
        TransitionKind::Crossfade {
            ratio: f64::from(percent) / 100.0
        }
    ));
}

#[when(expr = "I feather the {word} edge by {int} pixels")]
fn feather(world: &mut World, edge: String, width: f64) {
    let item = world.item();
    let edge = match edge.as_str() {
        "left" => Edge::Left,
        "right" => Edge::Right,
        "top" => Edge::Top,
        _ => Edge::Bottom,
    };
    let feather = Feather {
        left: edge == Edge::Left,
        right: edge == Edge::Right,
        top: edge == Edge::Top,
        bottom: edge == Edge::Bottom,
        width,
        curve: Curve::Linear,
    };
    assert!(world.editor.set_feather(item, Some(feather)));
}

#[when(expr = "I strip image {int}'s metadata")]
fn strip(world: &mut World, id: u32) {
    assert!(world.editor.strip_metadata(ItemId(id)));
}

#[when(expr = "I scroll up with Ctrl at {int}, {int}")]
fn ctrl_scroll(world: &mut World, x: f64, y: f64) {
    let screen = world.screen(x, y);
    world.remembered = Some(screen);
    world.editor.scroll(
        screen,
        0.0,
        -1.0,
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    );
}

#[given(expr = "I remember where image {int} is on screen")]
fn remember(world: &mut World, id: u32) {
    let item = *world
        .editor
        .display_state()
        .item(ItemId(id))
        .expect("image");
    world.remembered = Some(
        world
            .editor
            .view()
            .to_screen(Point::new(f64::from(item.x), f64::from(item.y))),
    );
}

#[then(expr = "image {int} is where I remembered on screen")]
fn where_remembered(world: &mut World, id: u32) {
    let item = *world
        .editor
        .display_state()
        .item(ItemId(id))
        .expect("image");
    let now = world
        .editor
        .view()
        .to_screen(Point::new(f64::from(item.x), f64::from(item.y)));
    let before = world.remembered.expect("remembered");
    assert!(now.distance(before) < 0.5, "{now:?} vs {before:?}");
}

#[then(expr = "canvas point {int}, {int} is still under the pointer")]
fn under_pointer(world: &mut World, x: f64, y: f64) {
    let screen = world.remembered.expect("pointer");
    let p = world.editor.view().to_canvas(screen);
    assert!(p.distance(Point::new(x, y)) < 1e-6, "{p:?}");
}

#[then(expr = "the zoom is {int}%")]
fn zoom_is(world: &mut World, percent: f64) {
    assert_eq!((world.editor.view().zoom * 100.0).round(), percent);
}

#[then("the canvas fits the view")]
fn fits(world: &mut World) {
    assert!(world.editor.view().fit);
}

#[then("undo is available")]
fn undo_available(world: &mut World) {
    assert!(world.editor.can_undo());
}

#[then("undo is not available")]
fn undo_unavailable(world: &mut World) {
    assert!(!world.editor.can_undo());
    assert_eq!(world.editor.undo_label(), None);
}

#[then("redo is available")]
fn redo_available(world: &mut World) {
    assert!(world.editor.can_redo());
}

#[then("redo is not available")]
fn redo_unavailable(world: &mut World) {
    assert!(!world.editor.can_redo());
    assert_eq!(world.editor.redo_label(), None);
}

#[then(regex = r"^the menu has (.+)$")]
fn menu_has(world: &mut World, labels: String) {
    for label in labels.split('"').skip(1).step_by(2) {
        assert!(
            world.menu.iter().any(|e| e.label == label),
            "no {label:?} in {:?}",
            world.menu.iter().map(|e| &e.label).collect::<Vec<_>>()
        );
    }
}

#[then(expr = "the menu does not have {string}")]
fn menu_lacks(world: &mut World, label: String) {
    assert!(!world.menu.iter().any(|e| e.label == label));
}

#[then(expr = "a vertical guide is shown at {int}")]
fn guide(world: &mut World, at: f64) {
    let guides = world.editor.guides();
    assert!(
        guides
            .iter()
            .any(|g| g.vertical && (g.at - at).abs() < 1e-6),
        "{guides:?}"
    );
}

#[then(expr = "the default feather width is {int}")]
fn default_feather(world: &mut World, width: f64) {
    let item = world.item();
    assert_eq!(world.editor.default_feather(item).width, width);
}

#[then(expr = "the EXIF notice is {string}")]
fn exif_notice(world: &mut World, text: String) {
    assert_eq!(world.editor.exif_notice().as_deref(), Some(text.as_str()));
}

#[then("there is no EXIF notice")]
fn no_exif_notice(world: &mut World) {
    assert_eq!(world.editor.exif_notice(), None);
}

#[then(expr = "the EXIF viewer opens on image {int}")]
fn viewer_on(world: &mut World, id: u32) {
    assert_eq!(world.editor.exif_viewer_item(), Some(ItemId(id)));
}

#[then(expr = "the EXIF viewer can switch between images {int} and {int}")]
fn viewer_items(world: &mut World, a: u32, b: u32) {
    assert_eq!(world.editor.items_with_exif(), vec![ItemId(a), ItemId(b)]);
}

#[then(expr = "the EXIF viewer shows {string} in group {string}")]
fn viewer_shows(world: &mut World, name: String, group: String) {
    let item = world.editor.exif_viewer_item().expect("viewer item");
    let entries = world.editor.exif_entries(item);
    assert!(
        entries
            .iter()
            .any(|e| e.name == name && e.group.label() == group),
        "{entries:?}"
    );
}

#[given("the screen is resampled in the background")]
fn background_resampling(world: &mut World) {
    world.editor.set_background_rendering(true);
}

#[when("the canvas is drawn")]
fn canvas_drawn(world: &mut World) {
    world.editor.frame();
}

#[then("the frame is drawn while images are still being resampled")]
fn drawn_while_resampling(world: &mut World) {
    world.editor.frame();
    assert!(world.editor.background_busy());
}

#[when("the background resampling finishes")]
fn resampling_finishes(world: &mut World) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while world.editor.background_busy() {
        assert!(
            std::time::Instant::now() < deadline,
            "resampling never finished"
        );
        world.editor.poll_background();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[then("the frame matches one drawn without background resampling")]
fn frame_matches_foreground(world: &mut World) {
    let path = world.path("matches.stitchpane");
    world.editor.save_as(&path).expect("save project");
    let view = *world.editor.view();
    let mut other = stitchpane_core::editor::Editor::new();
    other.set_viewport(view.viewport.0, view.viewport.1);
    other.open(&path).expect("open project");
    other.set_view(view.zoom, view.pan);
    other.select(world.editor.selection());
    assert!(world.editor.frame() == other.frame());
}

#[when(expr = "I start smearing image {int}")]
fn start_smearing(world: &mut World, id: u32) {
    assert!(world.editor.execute(Command::Smear(ItemId(id))));
}

#[when(expr = "I set the brush to {word} {int} pixels wide at {int}% strength")]
fn set_brush(world: &mut World, kind: String, size: f64, strength: f64) {
    let kind = StrokeKind::SMEAR
        .into_iter()
        .find(|k| k.label().eq_ignore_ascii_case(&kind))
        .unwrap_or_else(|| panic!("no smear kind {kind}"));
    world.editor.set_smear_kind(kind);
    world.editor.set_smear_size(size);
    world.editor.set_smear_strength(strength / 100.0);
}

#[then(expr = "the brush is {word} {int} pixels wide at {int}% strength")]
fn brush_is(world: &mut World, kind: String, size: f64, strength: f64) {
    let tool = world.editor.smear_tool().expect("smear tool active");
    assert!(tool.kind.label().eq_ignore_ascii_case(&kind), "{tool:?}");
    assert_eq!(tool.size, size);
    assert!((tool.strength * 100.0 - strength).abs() < 1e-9, "{tool:?}");
}

#[then(expr = "the smear tool is active on image {int}")]
fn smear_active(world: &mut World, id: u32) {
    assert_eq!(world.editor.smear_tool().map(|t| t.item), Some(ItemId(id)));
}

#[then("the smear tool is not active")]
fn smear_inactive(world: &mut World) {
    assert_eq!(world.editor.smear_tool(), None);
}

#[then(expr = "the screen at canvas point {int}, {int} is no longer {string}")]
fn screen_changed(world: &mut World, x: f64, y: f64, name: String) {
    let p = world.screen(x, y);
    let frame = world.editor.frame();
    let pixel = frame.get_pixel(p.x as u32, p.y as u32).0;
    assert!(
        !super::close(pixel, super::color(&name), 12),
        "{pixel:?} is {name}"
    );
}

#[then(expr = "the zoom menu has {string}, {string}, {string} and {string}")]
fn zoom_menu(world: &mut World, a: String, b: String, c: String, d: String) {
    let _ = world;
    let labels: Vec<String> = stitchpane_core::editor::Editor::zoom_menu()
        .into_iter()
        .map(|e| e.label)
        .collect();
    assert_eq!(labels, [a, b, c, d]);
}
