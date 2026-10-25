pub mod command;
mod geometry;
pub mod parser;
pub mod placement;
pub use terminal_kernel::protocols::pty_filter;
pub mod storage;
pub mod worker;

pub use command::{
  ImagePlacement, KittyAction, KittyCommand, KittyDelete, KittyFormat, KittyResponse,
  KittyResponseContext, KittyTransmission, RawGraphicsCommand, RawGraphicsEvent, StoredImage,
  VisiblePlacement,
};
pub use parser::KittyParser;
pub use placement::PlacementManager;
#[cfg(unix)]
pub use pty_filter::GraphicsPtyFilter;
#[cfg(not(unix))]
pub use pty_filter::{WindowsDsrCursorFn, WindowsDsrFilter};
pub use storage::{KittyImageStorage, PreparedImage, prepare_image, validate_image};
#[cfg(test)]
pub(crate) use worker::spawn_test_graphics_worker as spawn_graphics_worker;
pub use worker::{GRAPHICS_BATCH_SIZE, PreparedGraphicsEvent, graphics_worker};
