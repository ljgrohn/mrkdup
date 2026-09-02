use super::*;

fn titles(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn text(segs: &[Segment]) -> String {
    segs.iter().map(|s| s.text.as_str()).collect()
}

#[test]
fn after_close_picks_the_left_neighbour_of_the_active_tab() {
    assert_eq!(after_close(2, 2, 3), 1);
    assert_eq!(after_close(0, 0, 2), 0); // first closed: the new first
    assert_eq!(after_close(0, 0, 0), 0); // nothing left
}

#[test]
fn after_close_keeps_the_active_tab_when_another_closes() {
    assert_eq!(after_close(2, 0, 3), 1); // shifted down
    assert_eq!(after_close(0, 2, 3), 0); // untouched
    assert_eq!(after_close(1, 3, 3), 1);
}

#[test]
fn bar_paints_title_and_close_per_tab() {
    let segs = layout_bar(&titles(&["a.md", "b.md*"]), 1, 40);
    assert_eq!(text(&segs), " a.md ×  b.md* × ");
    assert_eq!(segs.len(), 4);
    assert_eq!(
        (segs[0].x0, segs[0].x1, segs[0].index, segs[0].close),
        (0, 6, 0, false)
    );
    assert_eq!(
        (segs[1].x0, segs[1].x1, segs[1].index, segs[1].close),
        (6, 8, 0, true)
    );
    assert!(!segs[0].active && segs[2].active && segs[3].active);
    // the close glyph itself hits, its trailing space too
    assert!(segs[1].hit(6) && segs[1].hit(7) && !segs[1].hit(8));
}

#[test]
fn bar_scrolls_so_the_active_tab_is_visible() {
    // each tab is 8 cells; only two fit in 18
    let t = titles(&["a.md", "b.md", "c.md"]);
    let segs = layout_bar(&t, 2, 18);
    assert_eq!(text(&segs), " b.md ×  c.md × ");
    assert_eq!(segs[0].index, 1);
    // active first: nothing scrolls, the third is cut at the edge
    let segs = layout_bar(&t, 0, 18);
    assert_eq!(text(&segs), " a.md ×  b.md ×  c");
    assert_eq!(segs.last().unwrap().index, 2);
    assert!(!segs.last().unwrap().close);
}

#[test]
fn bar_is_empty_without_tabs() {
    assert!(layout_bar(&[], 0, 40).is_empty());
}
