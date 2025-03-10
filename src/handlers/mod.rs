use std::sync::Arc;

use crate::AppData;
use crate::AppError;
use crate::Menu;

pub mod auth;
pub mod wallet;
pub mod project;
pub mod snipe;
pub mod main;
pub mod trade;
pub mod bumps;

pub trait Handler {
    fn handle(&self, app_data: &Arc<AppData>) -> impl std::future::Future<Output = Result<Option<Menu>, (Menu, AppError)>> + Send;
}