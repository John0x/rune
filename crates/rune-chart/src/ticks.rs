//! Tick values and their labels.
//!
//! Number ticks fall on nice steps (1, 2 or 5 × 10ᵏ, as d3's `ticks`), computed so that values
//! like 0.3 come out as `0.3`, not `0.30000000000000004`. Time ticks fall on calendar boundaries
//! in a time zone (whole minutes, hours, local midnights, the first of a month), so they stay
//! aligned across daylight-saving changes, and their labels show the largest unit that changed.

mod format;
mod interval;
mod label;
mod step;

pub use format::format_compact;
pub use format::format_number;
pub use format::precision;
pub use interval::TimeInterval;
pub use interval::TimeUnit;
pub use interval::time_ticks;
pub use label::format_time;
pub use step::nice;
pub use step::tick_step;
pub use step::ticks;

/// Upper bound on the ticks any function here returns, so a huge `count` can't allocate without
/// limit.
pub const MAX_TICKS: usize = 10_000;
