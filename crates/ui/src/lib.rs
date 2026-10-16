//! AirFrame's theme, icons and shared widgets.
//!
//! Everything visual that more than one screen needs lives here, so the
//! application crate only describes layout and behaviour.

pub mod icons;
pub mod logo;
pub mod theme;
pub mod widgets;

pub use icons::Icon;
pub use theme::{palette, protocol_color};
pub use widgets::{
    State, badge, badge_size, elide, icon_button, icon_toggle, label, solid_badge, state_pill,
};
