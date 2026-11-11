//! A menu drawn in the app's own look keeps rune-kit's machinery: its surface, row metrics and
//! row content replaced, its rows still menu items with their keys, submenus beside them.

use rune::{Color, Cx, El, Env, Runtime, el, text};
use rune_kit::{Kit, Menu, MenuEntry, MenuItem, root};

fn view(_: &(), _: &mut Cx<()>) -> El {
    let k = Kit::neutral(true);
    let mut layer = Vec::new();
    let sub =
        vec![MenuEntry::Custom(el().key("swatches").h(20.0)), MenuEntry::item(MenuItem::new("Deeper").key("deeper"))];
    let body = Menu::new("file")
        .trigger(text("File"))
        .item(MenuItem::new("Copy").key("copy").content(text("Copy").key("drawn")))
        .item(MenuItem::new("More").submenu(sub, true))
        .surface(|frame, depth| frame.key(format!("surface{depth}")).fill(Color::WHITE).w(200.0 - 40.0 * depth as f32))
        .row_height(26.0)
        .padding(5.0)
        .submenu_offset(3.0, -5.0)
        .open(true)
        .build(&k, &mut layer);
    root(rune::col().w_full().h_full().child(el().key("anchor").size(10.0, 10.0)).child(body), layer)
}

fn element(rt: &Runtime<()>, key: &str) -> rune::runtime::ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("{key}"))
}

#[test]
fn an_app_draws_the_surface_and_rows_rune_kit_lays_out() {
    let mut rt = Runtime::new((), view, (480.0, 320.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let (menu, sub) = (element(&rt, "surface0"), element(&rt, "surface1"));
    assert_eq!((menu.rect[2], sub.rect[2]), (200.0, 160.0), "each depth takes its own surface");
    let copy = element(&rt, "copy");
    assert_eq!(copy.rect[3], 26.0, "rows are as high as asked");
    assert!(element(&rt, "drawn").path.starts_with(&copy.path), "the row holds the drawn content");
    assert!(
        !rt.elements().iter().any(|e| e.path.starts_with(&copy.path) && e.key.as_deref() == Some("label")),
        "drawn content replaces the label"
    );
    let more = element(&rt, "file-item1");
    let first = element(&rt, "swatches");
    assert_eq!(first.rect[1], more.rect[1], "the submenu's first row lines up with its item");
    assert_eq!(sub.rect[0], more.rect[0] + more.rect[2] + 3.0, "the submenu opens past its item");
    element(&rt, "deeper");
}
