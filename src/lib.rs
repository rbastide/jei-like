//! Enough Items: a JEI-style item list and recipe viewer for Horizon.
//!
//! The work happens in the support module `src/viewer.rs`, which the launcher places inside the
//! game as `crate::game::enough_items`, and in two mixins (`src/mixins/`) that call it once a client
//! tick and on every event of the item panel.
use mod_api::{self, Hooks};

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {}
