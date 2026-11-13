//! Application layer — use cases and services. Thin over domain + repositories;
//! owns the transaction boundary for every mutation.

pub mod agent;
pub mod goals;
pub mod inbox;
pub mod planner;
pub mod portfolio;
pub mod projects;
pub mod settings;
pub mod tasks;
