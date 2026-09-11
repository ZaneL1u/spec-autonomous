//! Native SDD orchestration. Pure models and planners are independent of process/Git adapters.
pub mod cleanup;
pub mod config;
pub mod discovery;
pub mod engine;
pub mod git;
pub mod markdown;
pub mod model;
pub mod paths;
pub mod plan;
pub mod process;
pub mod progress;
pub mod provider;
pub mod runner;
pub mod skills;
pub mod state;
pub use discovery::{DetectionReport, Framework, detect};

pub mod provenance;
