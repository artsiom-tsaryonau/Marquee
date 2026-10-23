//! Native streaming control adapters. These do not open the library, start a
//! backend, launch a game, or implement a video receiver implicitly.
pub mod game_pilot;
pub mod receiver;
pub mod runtime;
pub mod viewer;
pub mod wolf;
