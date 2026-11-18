//! Hardware-free logic for the Framework 16 lighting app:
//! the raw HID message format ([`protocol`]), the combined
//! keyboard + macropad layout ([`layout`]) and the PC-rendered
//! "Linked" effects ([`effects`]).

pub mod builtin;
pub mod effects;
pub mod keys;
pub mod layout;
pub mod protocol;
