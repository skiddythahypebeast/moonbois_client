use std::time::Duration;

use dialoguer::theme::ColorfulTheme;
use crate::dialogue::loader::Loader;
use dialoguer::{FuzzySelect, Input};
use moonbois_core::{EnableBumpsParams, PumpfunBumpStatus};
use solana_sdk::native_token::LAMPORTS_PER_SOL;

use crate::{AppError, Menu};

use super::main::MainMenu;
use super::project::ProjectMenu;
use super::Handler;

pub enum BumpMenuOptions {
    Start,
    Stop,
    Back
}

impl ToString for BumpMenuOptions {
    fn to_string(&self) -> String {
        match self {
            BumpMenuOptions::Start => "StartBumps".to_string(),
            BumpMenuOptions::Stop => "StopBumps".to_string(),
            BumpMenuOptions::Back => "Back".to_string()
        }
    }
}

impl From<usize> for BumpMenuOptions {
    fn from(value: usize) -> Self {
        match value {
            0 => BumpMenuOptions::Start,
            1 => BumpMenuOptions::Stop,
            2 => BumpMenuOptions::Back,
            _ => panic!("Received invalid bump menu index")
        }
    }
}

pub struct BumpMenu;
impl Handler for BumpMenu {
    async fn handle(&self, _app_data: &std::sync::Arc<crate::AppData>) -> Result<Option<crate::Menu>, (crate::Menu, crate::AppError)> {
        let selection = match FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Bumps menu")
            .default(0)
            .items(vec![
                BumpMenuOptions::Start,
                BumpMenuOptions::Stop,
                BumpMenuOptions::Back
            ])
            .interact() {
            Ok(selection) => selection,
            Err(err) => {
                return Err((Menu::ProjectMenu(ProjectMenu), AppError::from(err)));
            }
        };

        match BumpMenuOptions::from(selection) {
            BumpMenuOptions::Start => return Ok(Some(Menu::StartBumps(StartBumps))),
            BumpMenuOptions::Stop => return Ok(Some(Menu::StopBumps(StopBumps))),
            BumpMenuOptions::Back => return Ok(Some(Menu::ProjectMenu(ProjectMenu)))
        }
    }
}

pub struct StartBumps;
impl Handler for StartBumps {
    async fn handle(&self, app_data: &std::sync::Arc<crate::AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let active_project_read = app_data.active_project.read().await; 
        let project_id = match &active_project_read.0 {
            Some(p) => p.clone(),
            None => return Err((Menu::Main(MainMenu), AppError::ProjectNotFound))
        };
        drop(active_project_read);

        let bump_interval: Duration = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter bump interval (seconds)")
            .default(3)
            .interact_text()
            .unwrap() {
                Some(seconds) => Duration::from_secs(seconds),
                None => return Ok(Some(Menu::Bump(BumpMenu)))
            };

        let bump_amount: u64 = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter bump amount (sol)")
            .default(0.015)
            .interact_text()
            .unwrap() {
                Some(wallet_count) => (wallet_count * LAMPORTS_PER_SOL as f64) as u64,
                None => return Ok(Some(Menu::Bump(BumpMenu)))
            };

        let params = EnableBumpsParams {
            bump_amount,
            bump_interval
        };

        let rpc_client = app_data.rpc_client.read().await;
        Loader::new()
            .with_prompt("start_bumps in progress")
            .interact(rpc_client.enable_bumps(project_id, params))
            .await
            .map_err(|err| (Menu::Bump(BumpMenu), AppError::from(err)))?;
        drop(rpc_client);

        let mut bump_status = app_data.bump_status.write().await;
        bump_status.0 = Some(PumpfunBumpStatus::Pending);
        drop(bump_status);

        Ok(Some(Menu::Bump(BumpMenu)))
    }
}

pub struct StopBumps;
impl Handler for StopBumps {
    async fn handle(&self, app_data: &std::sync::Arc<crate::AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let rpc_client = app_data.rpc_client.read().await;
        Loader::new()
            .with_prompt("stop_bumps in progress")
            .interact(rpc_client.disable_bumps())
            .await
            .map_err(|err| (Menu::Bump(BumpMenu), AppError::from(err)))?;
        drop(rpc_client);

        Ok(Some(Menu::Bump(BumpMenu)))
    }
}