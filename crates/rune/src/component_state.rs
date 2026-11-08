//! State a component keeps for itself, across view runs, without the app storing it: a chart's
//! zoom and hover, a control's animation values.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

/// Entries kept per thread before the oldest is dropped.
const KEPT: usize = 4096;

type Key = (u64, TypeId, String);

#[derive(Default)]
struct Slots {
    values: HashMap<Key, Rc<dyn Any>>,
    order: VecDeque<Key>,
}

thread_local! {
    static SLOTS: RefCell<Slots> = RefCell::new(Slots::default());
}

/// The `T` kept under `key` for the window whose view is being built: made with
/// `T::default()` the first time, the same `Rc` on every later view run. Hold `Cell`s,
/// `RefCell`s or [`Live`](crate::Live) values in `T` and change them from `_any` handlers
/// ([`El::on_drag_any`](crate::El::on_drag_any), [`El::on_wheel_any`](crate::El::on_wheel_any),
/// [`El::on_pointer_move_any`](crate::El::on_pointer_move_any)), calling `Tx::refresh` or
/// changing a `Live` for the view to see it.
///
/// Keys are per window and per type, so two components may use the same key with different
/// state types. A component used twice in one view needs two keys, as an element key does.
/// At most 4,096 slots are kept per thread; past that the oldest goes, starting over from
/// `T::default()` if asked for again.
pub fn component_state<T: Default + 'static>(key: &str) -> Rc<T> {
    let id = (crate::platform::scroll_bars::current_runtime_id(), TypeId::of::<T>(), key.to_owned());
    SLOTS.with(|slots| {
        let mut slots = slots.borrow_mut();
        if let Some(found) = slots.values.get(&id).and_then(|v| v.clone().downcast::<T>().ok()) {
            return found;
        }
        if slots.order.len() >= KEPT
            && let Some(oldest) = slots.order.pop_front()
        {
            slots.values.remove(&oldest);
        }
        let made = Rc::new(T::default());
        slots.values.insert(id.clone(), made.clone());
        slots.order.push_back(id);
        made
    })
}

#[cfg(test)]
#[path = "component_state_tests.rs"]
mod tests;
