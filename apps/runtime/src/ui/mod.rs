pub mod startup;
pub mod editor;
pub mod dock;
mod menu_bar;
mod hierarchy;
mod inspector;
mod console;
pub mod viewport;
mod dialogs;
mod undo_redo;
pub mod frame_graph_overlay;

pub use startup::startup_screen;
pub use editor::editor_screen;
pub use frame_graph_overlay::show_frame_graph_overlay;
