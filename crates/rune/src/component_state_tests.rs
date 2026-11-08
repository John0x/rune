use std::cell::Cell;

use super::*;

#[test]
fn a_key_keeps_its_state_and_types_keep_theirs_apart() {
    let zoom = component_state::<Cell<f32>>("chart");
    zoom.set(2.0);
    assert_eq!(component_state::<Cell<f32>>("chart").get(), 2.0, "the same slot on the next run");
    assert_eq!(component_state::<Cell<u32>>("chart").get(), 0, "another type under the same key");
    assert_eq!(component_state::<Cell<f32>>("other").get(), 0.0);
}

#[test]
fn the_oldest_slot_goes_past_the_cap() {
    component_state::<Cell<u8>>("first").set(7);
    for i in 0..KEPT {
        component_state::<Cell<u8>>(&format!("slot-{i}"));
    }
    assert_eq!(component_state::<Cell<u8>>("first").get(), 0, "made again");
}
