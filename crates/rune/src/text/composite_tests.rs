use super::*;
use crate::{BlockAttributes, BlockKind, TextEdit, text_input};

fn spec() -> crate::el::TextSpec {
    let mut e = TextEdit::new("before wraps over several words\ncode words words words\nafter").rich();
    e.set_logical_block(e.line_range(1), BlockKind::Code, BlockAttributes::default()).unwrap();
    (*text_input(&e, TextStyle::new(16.0, 400.0)).text.take().unwrap()).clone()
}

fn geometry(l: &Layout<Ink>) -> Vec<(std::ops::Range<usize>, [f32; 6])> {
    l.lines().map(|l| {
        let m = l.metrics();
        (l.text_range(), [m.offset, m.inline_min_coord, m.baseline, m.advance, m.block_min_coord, m.block_max_coord])
    }).collect()
}

fn id() -> crate::BlockId { TextEdit::new("").logical_blocks().next().unwrap().id }

#[test]
fn an_empty_slot_table_keeps_the_identical_layout_and_measurement() {
    let mut t = TextSystem::default();
    let s = spec();
    let a = t.layout_for_spec(&s, 2.0, Some(240.0), Alignment::Start);
    let b = t.layout_composite(&s, &[], 2.0, Some(240.0), Alignment::Start);
    assert!(Rc::ptr_eq(&a, &b));
    assert_eq!(geometry(&a), geometry(&b));
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
}

#[test]
fn a_header_reserves_height_without_adding_text_or_selection_boxes() {
    let mut t = TextSystem::default();
    let s = spec();
    let a = t.layout_for_spec(&s, 2.0, Some(240.0), Alignment::Start);
    let slots = [BlockSlot { block: id(), paragraph: 1, position: SlotPosition::Header, width: 120.0, height: 25.0, rtl: false }];
    let b = t.layout_composite(&s, &slots, 2.0, Some(240.0), Alignment::Start);
    for (old, new) in a.lines().zip(b.lines()) {
        assert_eq!(old.text_range(), new.text_range());
        let delta = if old.text_range().start >= s.content.find("code").unwrap() { 50.0 } else { 0.0 };
        assert_eq!(new.metrics().baseline - old.metrics().baseline, delta);
    }
    let mut measured = s.clone();
    measured.cold.block_slots = slots.to_vec();
    assert_eq!(measured.measured_height(&b, 2.0) - s.measured_height(&a, 2.0), 50.0);
    let boxes = super::super::range_boxes(&b, 0, s.content.len(), true);
    let first = b.lines().find(|l| l.text_range().start == s.content.find("code").unwrap()).unwrap();
    let gap_end = first.metrics().block_min_coord;
    assert!(boxes.iter().all(|(r, _)| r.y1 <= (gap_end - 50.0) as f64 || r.y0 >= gap_end as f64));
}

#[test]
fn leading_dimensions_and_header_heights_are_part_of_the_layout_cache_key() {
    let mut t = TextSystem::default();
    let s = spec();
    let mut slots = [BlockSlot { block: id(), paragraph: 1, position: SlotPosition::Leading, width: 20.0, height: 24.0, rtl: false }];
    let a = t.layout_composite(&s, &slots, 1.0, Some(140.0), Alignment::Start);
    slots[0].width = 90.0;
    let b = t.layout_composite(&s, &slots, 1.0, Some(140.0), Alignment::Start);
    assert!(!Rc::ptr_eq(&a, &b));
    assert_ne!(geometry(&a), geometry(&b));
    slots[0].position = SlotPosition::Header;
    let c = t.layout_composite(&s, &slots, 1.0, Some(140.0), Alignment::Start);
    slots[0].height += 20.0;
    let d = t.layout_composite(&s, &slots, 1.0, Some(140.0), Alignment::Start);
    assert_eq!(d.lines().last().unwrap().metrics().baseline - c.lines().last().unwrap().metrics().baseline, 20.0);
    let narrow = t.layout_composite(&s, &slots, 1.0, Some(90.0), Alignment::Start);
    assert!(narrow.len() > d.len());
    assert!(Rc::ptr_eq(&d, &t.layout_composite(&s, &slots, 1.0, Some(140.0), Alignment::Start)));
}
