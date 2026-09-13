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
pub mod skills;
pub mod state;
pub mod verification_preflight;
pub mod work_packet;
pub use discovery::{DetectionReport, Framework, detect};

pub mod provenance;

pub mod capabilities;

pub mod mcp;

pub mod skills_mcp;

pub mod run_revision;
