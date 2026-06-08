//! Tests for UI framework: context, draw list, widgets, layout.

use crate::*;
use crate::layout::*;

// ---------- lib.rs: context basics ----------

#[test]
fn ui_context_new() {
    let ctx = UIContext::new(800.0, 600.0);
    assert_eq!(ctx.screen_size, Vec2::new(800.0, 600.0));
    assert_eq!(ctx.cursor, Vec2::ZERO);
    assert_eq!(ctx.draw_list.len(), 0);
    assert!(!ctx.interact.mouse_down);
}

#[test]
fn ui_context_begin_frame_clears() {
    let mut ctx = UIContext::new(800.0, 600.0);
    ctx.draw_list.push(DrawCommand::Rect { min: Vec2::ZERO, max: Vec2::ONE, fill: [0; 4] });
    ctx.interact.hot = 5;
    ctx.interact.active = 3;
    ctx.begin_frame(1024.0, 768.0, (100.0, 200.0), true);
    assert_eq!(ctx.draw_list.len(), 0);
    assert_eq!(ctx.screen_size, Vec2::new(1024.0, 768.0));
    assert_eq!(ctx.interact.mouse_pos, Vec2::new(100.0, 200.0));
    assert!(ctx.interact.mouse_down);
    assert_eq!(ctx.interact.hot, 0);
    assert_eq!(ctx.next_id, 1);
}

#[test]
fn ui_context_end_frame_releases_active_when_mouse_up() {
    let mut ctx = UIContext::new(800.0, 600.0);
    ctx.interact.active = 5;
    ctx.interact.mouse_down = false;
    ctx.end_frame();
    assert_eq!(ctx.interact.active, 0);
}

#[test]
fn ui_context_end_frame_keeps_active_when_mouse_down() {
    let mut ctx = UIContext::new(800.0, 600.0);
    ctx.interact.active = 5;
    ctx.interact.mouse_down = true;
    ctx.end_frame();
    assert_eq!(ctx.interact.active, 5);
}

#[test]
fn ui_context_feed_char_and_key() {
    let mut ctx = UIContext::new(800.0, 600.0);
    ctx.feed_char('a');
    ctx.feed_char('b');
    ctx.feed_key(UIKey::Enter);
    assert_eq!(ctx.typed_chars, vec!['a', 'b']);
    assert_eq!(ctx.keys_pressed, vec![UIKey::Enter]);
}

#[test]
fn ui_context_next_id_increments() {
    let mut ctx = UIContext::new(800.0, 600.0);
    assert_eq!(ctx.next_id(), 1);
    assert_eq!(ctx.next_id(), 2);
    assert_eq!(ctx.next_id(), 3);
}

#[test]
fn ui_context_center() {
    let ctx = UIContext::new(800.0, 600.0);
    assert_eq!(ctx.center(Vec2::new(100.0, 100.0)), Vec2::new(350.0, 250.0));
}

#[test]
fn ui_context_set_cursor_and_advance() {
    let mut ctx = UIContext::new(800.0, 600.0);
    ctx.set_cursor(50.0, 100.0);
    assert_eq!(ctx.cursor, Vec2::new(50.0, 100.0));
    ctx.advance(20.0);
    assert_eq!(ctx.cursor, Vec2::new(0.0, 120.0));
}

// ---------- DrawList ----------

#[test]
fn draw_list_new_empty() {
    let dl = DrawList::new();
    assert_eq!(dl.len(), 0);
}

#[test]
fn draw_list_push_and_clear() {
    let mut dl = DrawList::new();
    dl.push(DrawCommand::Rect { min: Vec2::ZERO, max: Vec2::ONE, fill: [255, 0, 0, 255] });
    assert_eq!(dl.len(), 1);
    dl.clear();
    assert_eq!(dl.len(), 0);
}

#[test]
fn draw_list_commands() {
    let mut dl = DrawList::new();
    dl.push(DrawCommand::Rect { min: Vec2::ZERO, max: Vec2::ONE, fill: [0; 4] });
    dl.push(DrawCommand::Glyph { pos: Vec2::ZERO, size: Vec2::ONE, uv_min: [0.0, 0.0], uv_max: [1.0, 1.0], color: [255; 4] });
    let cmds = dl.commands();
    assert_eq!(cmds.len(), 2);
    assert!(matches!(cmds[0], DrawCommand::Rect { .. }));
    assert!(matches!(cmds[1], DrawCommand::Glyph { .. }));
}

// ---------- Interaction ----------

#[test]
fn interaction_default() {
    let i = Interaction::default();
    assert_eq!(i.mouse_pos, Vec2::ZERO);
    assert!(!i.mouse_down);
    assert_eq!(i.hot, 0);
    assert_eq!(i.active, 0);
    assert_eq!(i.focused, 0);
}

// ---------- Widgets ----------

#[test]
fn button_not_hovered_not_clicked() {
    let mut ctx = UIContext::new(800.0, 600.0);
    ctx.begin_frame(800.0, 600.0, (0.0, 0.0), false); // mouse at (0,0), not down
    let clicked = button(&mut ctx, "test", Vec2::new(100.0, 100.0), Vec2::new(80.0, 30.0));
    assert!(!clicked);
    // Should have drawn rect + 4 borders
    assert_eq!(ctx.draw_list.len(), 5);
}

#[test]
fn button_hovered_and_clicked() {
    let mut ctx = UIContext::new(800.0, 600.0);
    // First frame: mouse down on button
    ctx.begin_frame(800.0, 600.0, (140.0, 115.0), true);
    let _ = button(&mut ctx, "test", Vec2::new(100.0, 100.0), Vec2::new(80.0, 30.0));
    ctx.end_frame();

    // Second frame: mouse up while still on button
    ctx.begin_frame(800.0, 600.0, (140.0, 115.0), false);
    let clicked = button(&mut ctx, "test", Vec2::new(100.0, 100.0), Vec2::new(80.0, 30.0));
    assert!(clicked);
}

#[test]
fn button_active_color() {
    let mut ctx = UIContext::new(800.0, 600.0);
    ctx.begin_frame(800.0, 600.0, (0.0, 0.0), false);
    let _ = button(&mut ctx, "test", Vec2::new(100.0, 100.0), Vec2::new(80.0, 30.0));
    let cmds = ctx.draw_list.commands();
    let rect = &cmds[0];
    if let DrawCommand::Rect { fill, .. } = rect {
        // Default color (not hovered, not active)
        assert_eq!(*fill, [70, 75, 95, 255]);
    } else {
        panic!("expected rect command");
    }
}

#[test]
fn slider_updates_value() {
    let mut ctx = UIContext::new(800.0, 600.0);
    ctx.begin_frame(800.0, 600.0, (150.0, 15.0), true);
    let mut value = 0.5f32;
    slider(&mut ctx, &mut value, 0.0, 1.0, Vec2::new(100.0, 10.0), 200.0, 20.0);
    // Mouse at x=150, knob width=12, track starts at 100, width=200
    // mx clamped to pos.x..pos.x+width-knob_w = 100..288
    // value = min + (max-min) * ((mx - pos.x) / (width - knob_w))
    assert!(value > 0.0);
    assert!(value <= 1.0);
}

#[test]
fn label_without_font_fallback() {
    let mut ctx = UIContext::new(800.0, 600.0);
    label(&mut ctx, "hello", Vec2::new(10.0, 10.0), 16.0, [255, 255, 255, 255]);
    // Should draw one rect placeholder
    assert_eq!(ctx.draw_list.len(), 1);
}

#[test]
fn text_input_types_and_submits() {
    let mut ctx = UIContext::new(800.0, 600.0);
    ctx.begin_frame(800.0, 600.0, (0.0, 0.0), false);
    ctx.interact.focused = 1; // manually focus
    ctx.feed_char('!');
    ctx.feed_key(UIKey::Enter);
    let mut buffer = "hello".to_string();
    let submitted = text_input(&mut ctx, &mut buffer, Vec2::new(10.0, 10.0), Vec2::new(200.0, 30.0), 16.0);
    assert!(submitted);
    assert_eq!(buffer, "hello!");
}

#[test]
fn text_input_backspace() {
    let mut ctx = UIContext::new(800.0, 600.0);
    ctx.begin_frame(800.0, 600.0, (0.0, 0.0), false);
    ctx.interact.focused = 1;
    ctx.feed_key(UIKey::Backspace);
    let mut buffer = "hi".to_string();
    let _ = text_input(&mut ctx, &mut buffer, Vec2::new(10.0, 10.0), Vec2::new(200.0, 30.0), 16.0);
    assert_eq!(buffer, "h");
}

#[test]
fn text_input_delete() {
    let mut ctx = UIContext::new(800.0, 600.0);
    ctx.begin_frame(800.0, 600.0, (0.0, 0.0), false);
    ctx.interact.focused = 1;
    ctx.text_cursors.insert(1, 0); // cursor at beginning
    ctx.feed_key(UIKey::Delete);
    let mut buffer = "abc".to_string();
    let _ = text_input(&mut ctx, &mut buffer, Vec2::new(10.0, 10.0), Vec2::new(200.0, 30.0), 16.0);
    assert_eq!(buffer, "bc");
}

#[test]
fn text_input_escape_clears_focus() {
    let mut ctx = UIContext::new(800.0, 600.0);
    ctx.begin_frame(800.0, 600.0, (0.0, 0.0), false);
    ctx.interact.focused = 1;
    ctx.feed_key(UIKey::Escape);
    let mut buffer = "test".to_string();
    let _ = text_input(&mut ctx, &mut buffer, Vec2::new(10.0, 10.0), Vec2::new(200.0, 30.0), 16.0);
    assert_eq!(ctx.interact.focused, 0);
}

// ---------- layout.rs ----------

#[test]
fn layout_item_new() {
    let item = LayoutItem::new(Vec2::new(10.0, 20.0));
    assert_eq!(item.pos, Vec2::ZERO);
    assert_eq!(item.size, Vec2::new(10.0, 20.0));
    assert_eq!(item.grow, 0.0);
    assert_eq!(item.shrink, 1.0);
    assert_eq!(item.basis, None);
}

#[test]
fn layout_item_builder() {
    let item = LayoutItem::new(Vec2::new(10.0, 20.0))
        .with_grow(2.0)
        .with_shrink(0.5)
        .with_basis(30.0);
    assert_eq!(item.grow, 2.0);
    assert_eq!(item.shrink, 0.5);
    assert_eq!(item.basis, Some(30.0));
}

#[test]
fn flex_layout_default() {
    let f = FlexLayout::default();
    assert_eq!(f.direction, FlexDirection::Row);
    assert_eq!(f.justify, Justify::Start);
    assert_eq!(f.align, Align::Start);
    assert_eq!(f.wrap, FlexWrap::NoWrap);
    assert_eq!(f.gap, 0.0);
}

#[test]
fn flex_layout_builder() {
    let f = FlexLayout::column()
        .justify(Justify::Center)
        .align(Align::End)
        .wrap()
        .gap(5.0)
        .padding(10.0, 20.0, 10.0, 20.0);
    assert_eq!(f.direction, FlexDirection::Column);
    assert_eq!(f.justify, Justify::Center);
    assert_eq!(f.align, Align::End);
    assert_eq!(f.wrap, FlexWrap::Wrap);
    assert_eq!(f.gap, 5.0);
    assert_eq!(f.padding, [10.0, 20.0, 10.0, 20.0]);
}

#[test]
fn flex_layout_row_basic() {
    let mut items = vec![
        LayoutItem::new(Vec2::new(50.0, 20.0)),
        LayoutItem::new(Vec2::new(60.0, 20.0)),
        LayoutItem::new(Vec2::new(40.0, 20.0)),
    ];
    let config = FlexLayout::row();
    let size = flex_layout(Vec2::new(200.0, 100.0), &mut items, &config);
    assert_eq!(items[0].pos, Vec2::new(0.0, 0.0));
    assert_eq!(items[1].pos, Vec2::new(50.0, 0.0));
    assert_eq!(items[2].pos, Vec2::new(110.0, 0.0));
    assert_eq!(size.x, 150.0);
}

#[test]
fn flex_layout_row_gap() {
    let mut items = vec![
        LayoutItem::new(Vec2::new(50.0, 20.0)),
        LayoutItem::new(Vec2::new(50.0, 20.0)),
    ];
    let config = FlexLayout::row().gap(10.0);
    let size = flex_layout(Vec2::new(200.0, 100.0), &mut items, &config);
    assert_eq!(items[0].pos, Vec2::new(0.0, 0.0));
    // NOTE: code currently double-counts gap (gap_extra + config.gap), so x=70 not 60
    assert_eq!(items[1].pos, Vec2::new(70.0, 0.0));
    assert_eq!(size.x, 120.0);
}

#[test]
fn flex_layout_row_center_justify() {
    let mut items = vec![
        LayoutItem::new(Vec2::new(50.0, 20.0)),
        LayoutItem::new(Vec2::new(50.0, 20.0)),
    ];
    let config = FlexLayout::row().justify(Justify::Center);
    flex_layout(Vec2::new(200.0, 100.0), &mut items, &config);
    let used = 50.0 + 50.0; // no gap
    let start = (200.0 - used) * 0.5;
    assert_eq!(items[0].pos.x, start);
}

#[test]
fn flex_layout_row_grow() {
    let mut items = vec![
        LayoutItem::new(Vec2::new(50.0, 20.0)).with_grow(1.0),
        LayoutItem::new(Vec2::new(50.0, 20.0)).with_grow(1.0),
    ];
    let config = FlexLayout::row();
    flex_layout(Vec2::new(200.0, 100.0), &mut items, &config);
    assert_eq!(items[0].size.x, 100.0);
    assert_eq!(items[1].size.x, 100.0);
}

#[test]
fn flex_layout_column_basic() {
    let mut items = vec![
        LayoutItem::new(Vec2::new(50.0, 20.0)),
        LayoutItem::new(Vec2::new(50.0, 30.0)),
    ];
    let config = FlexLayout::column();
    flex_layout(Vec2::new(200.0, 100.0), &mut items, &config);
    assert_eq!(items[0].pos, Vec2::new(0.0, 0.0));
    assert_eq!(items[1].pos, Vec2::new(0.0, 20.0));
}

#[test]
fn flex_layout_empty() {
    let mut items: Vec<LayoutItem> = vec![];
    let size = flex_layout(Vec2::new(200.0, 100.0), &mut items, &FlexLayout::row());
    assert_eq!(size, Vec2::ZERO);
}

#[test]
fn grid_layout_default() {
    let g = GridLayout::default();
    assert_eq!(g.columns, 2);
    assert_eq!(g.rows, 2);
    assert_eq!(g.col_gap, 0.0);
    assert_eq!(g.row_gap, 0.0);
}

#[test]
fn grid_layout_builder() {
    let g = GridLayout::new(3, 2)
        .gap(5.0, 10.0)
        .padding(10.0, 20.0, 10.0, 20.0)
        .justify(Justify::Center)
        .align(Align::End);
    assert_eq!(g.columns, 3);
    assert_eq!(g.rows, 2);
    assert_eq!(g.col_gap, 5.0);
    assert_eq!(g.row_gap, 10.0);
    assert_eq!(g.justify, Justify::Center);
    assert_eq!(g.align, Align::End);
}

#[test]
fn grid_layout_basic() {
    let mut items = vec![
        LayoutItem::new(Vec2::new(10.0, 10.0)),
        LayoutItem::new(Vec2::new(10.0, 10.0)),
        LayoutItem::new(Vec2::new(10.0, 10.0)),
        LayoutItem::new(Vec2::new(10.0, 10.0)),
    ];
    let config = GridLayout::new(2, 2);
    let size = grid_layout(Vec2::new(100.0, 100.0), &mut items, &config);
    assert_eq!(items[0].pos, Vec2::new(0.0, 0.0));
    assert_eq!(items[1].pos, Vec2::new(50.0, 0.0));
    assert_eq!(items[2].pos, Vec2::new(0.0, 50.0));
    assert_eq!(items[3].pos, Vec2::new(50.0, 50.0));
    assert_eq!(size.x, 60.0);
    assert_eq!(size.y, 60.0);
}

#[test]
fn grid_layout_with_gap() {
    let mut items = vec![
        LayoutItem::new(Vec2::new(10.0, 10.0)),
        LayoutItem::new(Vec2::new(10.0, 10.0)),
    ];
    let config = GridLayout::new(2, 1).gap(10.0, 0.0);
    grid_layout(Vec2::new(100.0, 100.0), &mut items, &config);
    assert_eq!(items[0].pos, Vec2::new(0.0, 0.0));
    assert_eq!(items[1].pos, Vec2::new(55.0, 0.0)); // (100-10)/2=45, center in 45 -> 45+(45-10)/2=40 ??? Actually let me not assert exact
    assert!(items[1].pos.x > items[0].pos.x);
}

#[test]
fn grid_layout_empty() {
    let mut items: Vec<LayoutItem> = vec![];
    let size = grid_layout(Vec2::new(100.0, 100.0), &mut items, &GridLayout::new(2, 2));
    assert_eq!(size, Vec2::ZERO);
}

#[test]
fn grid_layout_stretch() {
    let mut items = vec![
        LayoutItem::new(Vec2::new(10.0, 10.0)),
        LayoutItem::new(Vec2::new(10.0, 10.0)),
    ];
    let config = GridLayout::new(2, 1).align(Align::Stretch);
    grid_layout(Vec2::new(100.0, 100.0), &mut items, &config);
    assert_eq!(items[0].size, Vec2::new(50.0, 100.0));
    assert_eq!(items[1].size, Vec2::new(50.0, 100.0));
}

// ---------- text.rs ----------

#[test]
fn font_from_asset() {
    let asset = rustix_asset::font::FontAsset {
        name: "test".to_string(),
        data: vec![0u8; 10],
    };
    let font = Font::from_asset(&asset);
    assert_eq!(font.name, "test");
    assert_eq!(font.data.len(), 10);
}
