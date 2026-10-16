//! AirFrame's protocol-agnostic core: the connection model, the session
//! contract every backend implements, and the on-disk store.
//!
//! The UI and the protocol backends both depend on this crate and on nothing
//! protocol-specific, which is what keeps `airframe-vnc`, `airframe-rdp` and
//! `airframe-spice` interchangeable.

pub mod connection;
pub mod session;
pub mod store;

pub use connection::{Connection, Protocol, RdpOptions, RdpSecurity, VncOptions};
pub use session::{
    CursorImage, FramePatch, KeyInput, SessionCommand, SessionEvent, SessionHandle, apply_patch,
    buttons,
};
pub use store::{Lang, Settings, Store, StoreData, StoreError, Theme};
