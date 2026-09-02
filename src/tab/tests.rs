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
        (segs[0].x0, segs[0].x1, segs[0].index, segs[0].part),
        (0, 6, 0, Part::Title)
    );
    assert_eq!(
        (segs[1].x0, segs[1].x1, segs[1].index, segs[1].part),
        (6, 8, 0, Part::Close)
    );
    assert!(!segs[0].active && segs[2].active && segs[3].active);
    // the close glyph itself hits, its trailing space too
    assert!(segs[1].hit(6) && segs[1].hit(7) && !segs[1].hit(8));
}

#[test]
fn bar_scrolls_so_the_active_tab_is_visible_with_edge_markers() {
    // each tab is 8 cells; with the two edge cells only two fit in 18
    let t = titles(&["a.md", "b.md", "c.md"]);
    let segs = layout_bar(&t, 2, 18);
    assert_eq!(text(&segs), "‹ b.md ×  c.md ×  ");
    assert_eq!((segs[0].part, segs[0].index), (Part::Prev, 1));
    assert_eq!(segs[1].index, 1);
    let last = segs.last().unwrap();
    assert_eq!(
        (last.part, last.text.as_str(), last.x0),
        (Part::Next, " ", 17)
    );
    // active first: nothing scrolls, the third is cut at the edge and
    // the right marker says so
    let segs = layout_bar(&t, 0, 18);
    assert_eq!(text(&segs), "  a.md ×  b.md × ›");
    assert_eq!(segs[0].text, " "); // nothing hidden on the left
    let last = segs.last().unwrap();
    assert_eq!((last.part, last.index), (Part::Next, 1));
    // active in the middle: markers on both sides
    let segs = layout_bar(&t, 1, 18);
    assert_eq!(text(&segs), "  a.md ×  b.md × ›");
    let segs = layout_bar(&t, 2, 10);
    assert_eq!(text(&segs), "‹ c.md ×  ");
}

#[test]
fn bar_without_overflow_has_no_edge_markers() {
    let segs = layout_bar(&titles(&["a.md", "b.md"]), 0, 16);
    assert_eq!(text(&segs), " a.md ×  b.md × ");
    assert!(segs
        .iter()
        .all(|s| matches!(s.part, Part::Title | Part::Close)));
}

#[test]
fn long_titles_are_cut_with_an_ellipsis() {
    let long = "a-very-long-file-name-indeed.md";
    let segs = layout_bar(&titles(&[long]), 0, 80);
    assert_eq!(segs[0].text, " a-very-long-file-na… ");
    assert_eq!(segs[0].text.chars().count(), MAX_TITLE + 2);
}

#[test]
fn bar_is_empty_without_tabs_or_width() {
    assert!(layout_bar(&[], 0, 40).is_empty());
    assert!(layout_bar(&titles(&["a.md"]), 0, 0).is_empty());
}
