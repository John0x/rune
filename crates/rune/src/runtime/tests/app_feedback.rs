use super::*;

#[derive(Default)]
struct Heard { hovers: Vec<bool>, log: Vec<&'static str> }

fn view(_: &Heard, _: &mut Cx<Heard>) -> El {
    col()
        .child(el().key("outer").size(100.0, 40.0).on_press(|s: &mut Heard| s.log.push("outer"))
            .child(el().key("inner").size(30.0, 20.0).on_press(|s: &mut Heard| s.log.push("inner"))))
        .child(el().key("plain").size(100.0, 40.0))
}

fn runtime() -> Runtime<Heard> {
    let mut rt = Runtime::new(Heard::default(), view, (100.0, 100.0), Env { system_dark: true, scale: 1.0 });
    rt.on_pressable_hover = Some(|s, inside, _| s.hovers.push(inside));
    rt.on_any_press = Some(|s, _| s.log.push("any"));
    rt.frame(0.0);
    rt
}

#[test]
fn the_app_hears_the_pointer_enter_each_pressable_and_leave_the_last() {
    let mut rt = runtime();
    rt.pointer_move(80.0, 30.0);
    rt.frame(0.0);
    assert_eq!(rt.state.hovers, [true], "entering a pressable");
    rt.pointer_move(85.0, 35.0);
    rt.frame(0.0);
    assert_eq!(rt.state.hovers, [true], "moving within it says nothing");
    rt.pointer_move(10.0, 10.0);
    rt.frame(0.0);
    assert_eq!(rt.state.hovers, [true, true], "a pressable nested in it is another");
    rt.pointer_move(50.0, 60.0);
    rt.frame(0.0);
    assert_eq!(rt.state.hovers, [true, true, false], "over nothing pressable");
    rt.pointer_move(50.0, 20.0);
    rt.pointer_leave();
    rt.frame(0.0);
    assert_eq!(rt.state.hovers.last(), Some(&false), "the pointer left the window");
}

#[test]
fn the_app_hears_every_press_before_the_element_does() {
    let mut rt = runtime();
    rt.pointer_down(10.0, 10.0);
    rt.pointer_up(10.0, 10.0);
    rt.frame(0.0);
    assert_eq!(rt.state.log, ["any", "inner"]);
}
