//! HTTP request handlers for agent endpoints.

pub mod error_codes;

pub mod admin;
pub mod agent;
pub mod agent_download;
pub mod backfill;
pub mod content;
pub mod download;
pub mod extract;
pub mod game;
pub mod gamesession;
pub mod hardware;
pub mod health;
pub mod install;
pub mod metrics;
pub mod option;
pub mod override_config;
pub mod priorities;
pub mod progress;
pub mod register;
pub mod repair;
pub mod size_estimate;
pub mod spawned;
pub mod uninstall;
pub mod update;
pub mod version;
