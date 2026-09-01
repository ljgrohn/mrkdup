use super::*;

fn lines(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn recomputes_on_first_call_only() {
    let mut cache = LayoutCache::new();
    let ls = lines(&["hello world"]);
    cache.ensure(&ls, 20, FileKind::Markdown);
    assert_eq!(cache.recomputes, 1);
    cache.ensure(&ls, 20, FileKind::Markdown);
    cache.ensure(&ls, 20, FileKind::Markdown);
    assert_eq!(cache.recomputes, 1, "unchanged inputs should reuse cache");
}

#[test]
fn width_change_forces_recompute() {
    let mut cache = LayoutCache::new();
    let ls = lines(&["hello world"]);
    cache.ensure(&ls, 20, FileKind::Markdown);
    cache.ensure(&ls, 10, FileKind::Markdown);
    assert_eq!(cache.recomputes, 2);
}

#[test]
fn file_kind_change_forces_recompute() {
    let mut cache = LayoutCache::new();
    let ls = lines(&["<p>hi</p>"]);
    cache.ensure(&ls, 20, FileKind::Markdown);
    cache.ensure(&ls, 20, FileKind::Html);
    assert_eq!(cache.recomputes, 2);
}

#[test]
fn invalidate_forces_recompute() {
    let mut cache = LayoutCache::new();
    let ls = lines(&["hello world"]);
    cache.ensure(&ls, 20, FileKind::Markdown);
    cache.invalidate();
    cache.ensure(&ls, 20, FileKind::Markdown);
    assert_eq!(cache.recomputes, 2);
}
