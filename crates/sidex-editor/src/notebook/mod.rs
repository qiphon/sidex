//! Notebook / cell model for Jupyter-style editing.

pub mod notebook_model;

pub use notebook_model::{CellKind, CellOutput, Notebook, NotebookCell};
