use std::ops::RangeInclusive;
use std::time::Duration;

use wasm_bindgen::JsCast;
use web_sys::{Document, Window};

pub fn range_intersection<T: Copy + Ord>(
    r1: RangeInclusive<T>,
    r2: RangeInclusive<T>,
) -> RangeInclusive<T> {
    let min = *r1.start().max(r2.start());
    let max = *r1.end().min(r2.end());

    min..=max
}

pub fn format_duration(dur: Duration) -> String {
    let sec = dur.as_secs();

    let min = sec / 60;
    let sec = sec % 60;

    format!("{min:02}:{sec:02}")
}

pub fn window() -> Window {
    web_sys::window().expect("window should exist")
}

pub fn document() -> Document {
    window().document().expect("document should exist")
}

pub fn get_element_by_id<T: JsCast>(id: impl AsRef<str>) -> T {
    _get_element_by_id(id.as_ref())
}

fn _get_element_by_id<T: JsCast>(id: &str) -> T {
    document()
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("element '{id}': not found"))
        .dyn_into()
        .unwrap_or_else(|_| {
            panic!(
                "element '{id}': cannot cast into `{}`",
                std::any::type_name::<T>()
            )
        })
}
