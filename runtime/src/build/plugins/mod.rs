// Shared adapter utilities for component conversion plugins
pub mod adapter;

#[cfg(feature = "plugin-javy")]
pub mod javy;
#[cfg(feature = "plugin-quickjs")]
pub mod quickjs;
#[cfg(feature = "plugin-assemblyscript")]
pub mod assemblyscript;
