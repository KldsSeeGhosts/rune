use super::*;
use crate::{BlockKind, BlockStyle, TextEdit, text_input};

fn spec(edit: &TextEdit, style: BlockStyle) -> crate::el::TextSpec {
    (*text_input(edit, TextStyle::new(16.0, 400.0).lh(24.0)).block_style(style).text.unwrap()).clone()
}

#[test]
fn unset_block_style_options_keep_the_previous_layout_and_heading_spans() {
    let mut edit = TextEdit::new("heading\n\nbody").rich();
    edit.set_block_kind(0, BlockKind::Heading { level: 1 });
    edit.set_block_kind(1, BlockKind::Rule);
    let style = BlockStyle::default();
    assert_eq!(style.checkbox_checked_fill, None);
    assert_eq!(style.checkbox_mark_color, None);
    assert_eq!(style.heading_line_height, [None; 6]);
    assert_eq!(style.heading_color, [None; 6]);
    assert_eq!(style.rule_margin_top, None);
    assert_eq!(style.rule_margin_bottom, None);
    let spec = spec(&edit, style);
    let mut ts = TextSystem::default();
    let old = ts.layout_keyed(spec.shape_key(2.0), &spec.content, &spec.spans,
        &spec.cold.blocks, &spec.cold.pills, spec.style, 2.0, Some(300.0),
        Alignment::Start, spec.wrap_anywhere, true, spec.base());
    let new = ts.layout_for_spec(&spec, 2.0, Some(300.0), Alignment::Start);
    assert!(Rc::ptr_eq(&old, &new));
    assert!(spec.spans.iter().all(|s| s.color.is_none()));
}

#[test]
fn each_heading_level_has_its_own_line_height_and_color_and_cache_key() {
    let mut edit = TextEdit::new("one\ntwo\nthree\nfour\nfive\nsix\nbody").rich();
    for level in 1..=6 { edit.set_block_kind(level as usize - 1, BlockKind::Heading { level }); }
    let red = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    let mut style = BlockStyle { heading_scale: [1.0; 6], ..Default::default() };
    style.heading_line_height = [Some(1.0), Some(1.25), Some(1.5), Some(1.75), Some(2.0), Some(2.25)];
    style.heading_color[5] = Some(red);
    let spec = spec(&edit, style);
    assert!(spec.spans.iter().any(|s| s.start as usize == edit.line_range(5).start && s.color == Some(red)));
    assert!(spec.spans.iter().filter(|s| s.start as usize != edit.line_range(5).start).all(|s| s.color.is_none()));
    let mut ts = TextSystem::default();
    let layout = ts.layout_for_spec(&spec, 2.0, Some(300.0), Alignment::Start);
    for (i, line) in layout.lines().take(6).enumerate() {
        assert!((line.metrics().line_height - (16.0 + i as f32 * 4.0) * 2.0).abs() < 0.1, "level {}: {:?}", i + 1, line.metrics());
    }
    assert!((layout.lines().last().unwrap().metrics().line_height - 48.0).abs() < 0.1);
    style.heading_line_height[0] = Some(3.0);
    let changed = super::block_style_tests::spec(&edit, style);
    let other = ts.layout_for_spec(&changed, 2.0, Some(300.0), Alignment::Start);
    assert!(!Rc::ptr_eq(&layout, &other));
    assert!((other.lines().next().unwrap().metrics().line_height - 96.0).abs() < 0.1);
    style.heading_scale[0] = 2.0;
    let scaled = super::block_style_tests::spec(&edit, style);
    let scaled = ts.layout_for_spec(&scaled, 2.0, Some(300.0), Alignment::Start);
    assert!((scaled.lines().next().unwrap().metrics().line_height - 192.0).abs() < 0.1, "{:?}", scaled.lines().next().unwrap().metrics());
}

#[test]
fn empty_and_single_heading_metrics_can_be_smaller_than_the_body_strut() {
    for source in ["", "heading"] {
        let mut edit = TextEdit::new(source).rich();
        edit.set_block_kind(0, BlockKind::Heading { level: 1 });
        let spec = spec(&edit, BlockStyle {
            heading_scale: [1.0; 6], heading_line_height: [Some(1.0); 6], ..Default::default()
        });
        let mut ts = TextSystem::default();
        let layout = ts.layout_for_spec(&spec, 2.0, Some(300.0), Alignment::Start);
        assert!((layout.lines().next().unwrap().metrics().line_height - 32.0).abs() < 0.1);
        assert!((spec.measured_height(&layout, 2.0) - 32.0).abs() < 0.1);
    }
}

#[test]
fn a_trailing_empty_paragraph_uses_its_own_heading_or_body_height() {
    for heading_line in [0, 1] {
        let mut edit = TextEdit::new("words\n").rich();
        edit.set_block_kind(heading_line, BlockKind::Heading { level: 1 });
        let spec = spec(&edit, BlockStyle {
            heading_scale: [1.0; 6], heading_line_height: [Some(1.0); 6], ..Default::default()
        });
        let mut ts = TextSystem::default();
        let layout = ts.layout_for_spec(&spec, 2.0, Some(300.0), Alignment::Start);
        for (i, line) in layout.lines().enumerate() {
            let expected = if i == heading_line { 32.0 } else { 48.0 };
            assert!((line.metrics().line_height - expected).abs() < 0.1, "{heading_line}/{i}: {:?}", line.metrics());
        }
        let cursor = parley::editing::Cursor::from_byte_index(&layout, edit.text.len(), parley::Affinity::Downstream);
        let rect = crate::text::cursor_geometry(&cursor, &layout, 1.0);
        let last = layout.lines().last().unwrap();
        let m = last.metrics();
        assert!((rect.y0 - f64::from(m.block_min_coord.min(m.content_block_min_coord))).abs() < 0.1, "{rect:?}, {m:?}");
        assert!((rect.y1 - f64::from(m.block_max_coord.max(m.content_block_max_coord))).abs() < 0.1, "{rect:?}, {m:?}");
        assert_eq!(crate::text::cursor_from_point(&layout, 0.0, last.metrics().baseline).index(), edit.text.len());
    }
}

#[test]
fn rule_margins_shift_rule_and_following_carets_and_are_measured_and_cached() {
    for source in ["above\n\nbelow", "above\n", ""] {
        let mut edit = TextEdit::new(source).rich();
        let rule = if source.is_empty() { 0 } else { 1 };
        edit.set_block_kind(rule, BlockKind::Rule);
        let base = spec(&edit, BlockStyle::default());
        let styled = spec(&edit, BlockStyle { rule_margin_top: Some(9.0), rule_margin_bottom: Some(13.0), ..Default::default() });
        let mut ts = TextSystem::default();
        let a = ts.layout_for_spec(&base, 2.0, Some(300.0), Alignment::Start);
        let b = ts.layout_for_spec(&styled, 2.0, Some(300.0), Alignment::Start);
        assert!((styled.measured_height(&b, 2.0) - base.measured_height(&a, 2.0) - 44.0).abs() < 0.1);
        for (i, (old, new)) in a.lines().zip(b.lines()).enumerate() {
            let expected = if i < rule { 0.0 } else if i == rule { 18.0 } else { 44.0 };
            assert!((new.metrics().baseline - old.metrics().baseline - expected).abs() < 0.1);
            assert_eq!(old.text_range(), new.text_range());
            let at = old.text_range().start;
            let ca = parley::editing::Cursor::from_byte_index(&a, at, parley::Affinity::Downstream);
            let cb = parley::editing::Cursor::from_byte_index(&b, at, parley::Affinity::Downstream);
            let dy = crate::text::cursor_geometry(&cb, &b, 1.0).y0 - crate::text::cursor_geometry(&ca, &a, 1.0).y0;
            assert!((dy - f64::from(expected)).abs() < 0.1);
        }
        let again = ts.layout_for_spec(&styled, 2.0, Some(300.0), Alignment::Start);
        assert!(Rc::ptr_eq(&b, &again));
    }
}

#[test]
fn rule_margins_and_widget_slots_add_without_affecting_document_ranges() {
    let mut edit = TextEdit::new("above\n\nbelow").rich();
    edit.set_block_kind(1, BlockKind::Rule);
    let base = spec(&edit, BlockStyle::default());
    let mut styled = spec(&edit, BlockStyle { rule_margin_top: Some(9.0), rule_margin_bottom: Some(13.0), ..Default::default() });
    styled.cold.block_slots.push(BlockSlot {
        block: edit.logical_blocks().nth(1).unwrap().id, paragraph: 1,
        position: SlotPosition::Header, width: 150.0, height: 24.0, rtl: false,
    });
    let mut ts = TextSystem::default();
    let a = ts.layout_for_spec(&base, 2.0, Some(300.0), Alignment::Start);
    let b = ts.layout_for_spec(&styled, 2.0, Some(300.0), Alignment::Start);
    assert!((styled.measured_height(&b, 2.0) - base.measured_height(&a, 2.0) - 92.0).abs() < 0.1);
    assert_eq!(a.lines().map(|l| l.text_range()).collect::<Vec<_>>(), b.lines().map(|l| l.text_range()).collect::<Vec<_>>());
}

#[test]
fn invalid_optional_spacing_inherits_without_changing_the_cache_key() {
    let mut edit = TextEdit::new("heading\n\nbody").rich();
    edit.set_block_kind(0, BlockKind::Heading { level: 1 });
    edit.set_block_kind(1, BlockKind::Rule);
    let base = spec(&edit, BlockStyle::default());
    let invalid = spec(&edit, BlockStyle {
        heading_line_height: [Some(f32::NAN); 6],
        rule_margin_top: Some(f32::INFINITY), rule_margin_bottom: Some(-20.0), ..Default::default()
    });
    let mut ts = TextSystem::default();
    let a = ts.layout_for_spec(&base, 2.0, Some(300.0), Alignment::Start);
    let b = ts.layout_for_spec(&invalid, 2.0, Some(300.0), Alignment::Start);
    assert!(Rc::ptr_eq(&a, &b));
}
