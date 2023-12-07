use serde::Serialize;

pub mod home;
pub mod models;
pub mod report;
pub mod running;
pub mod sessions;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Screen {
  HOME,
  MODELS,
  REPORT,
  SESSIONS,
  RUNNING,
}
