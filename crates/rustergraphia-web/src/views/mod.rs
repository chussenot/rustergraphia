//! The pages, rendered: each function mirrors one `views/*.erb` of the Ruby
//! app line by line, and the goldens hold it to the byte. `<%= %>` is `h()`,
//! `<%== %>` is pushed raw; a `<% %>` line alone on its line leaves nothing
//! behind, as Erubi trims it.

pub mod about;
pub mod index;
pub mod layout;
pub mod partials;
pub mod play;
pub mod result;
pub mod simple;

pub use layout::{render_layout, Frame};
