use dialoguer::theme::ColorfulTheme;
use crate::dialogue::loader::Loader;
use std::sync::Arc;

use dialoguer::Input;
use solana_sdk::pubkey::Pubkey;

use crate::AppData;
use crate::AppError;
use crate::Menu;

use super::main::MainMenu;
use super::project::ProjectMenu;
use super::Handler;

pub struct CreateSnipe;
impl Handler for CreateSnipe {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let sniper_count = match &app_data.user.read().await.0 {
            Some(user) => user.wallets.len(),
            None => return Err((Menu::Main(MainMenu), AppError::UserNotFound))
        };

        let wallet_count = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter wallet amount")
            .default(5)
            .validate_with(|val: &usize| -> Result<(), String> {
                if val > &sniper_count {
                    return Err("Wallet amount exceeds available wallets".to_string())
                };

                Ok(())
            })
            .interact_text()
            .unwrap() {
                Some(wallet_count) => wallet_count,
                None => return Ok(Some(Menu::Main(MainMenu)))
            };

        let deployer: Pubkey = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter the deployer address")
            .interact_text()
            .unwrap() {
                Some(deployer) => deployer,
                None => return Ok(Some(Menu::Main(MainMenu)))
            };

        let rpc_client = app_data.rpc_client.read().await;
        let result = Loader::new()
            .with_prompt("Creating snipe")
            .interact(rpc_client.create_snipe(deployer, wallet_count))
            .await;

        let pending_snipe = match result {
            Ok(result) => result,
            Err(err) => return Err((Menu::Main(MainMenu), AppError::from(err)))
        };

        let deployer = pending_snipe.deployer;

        let loader = Loader::new()
            .with_prompt("Snipe pending")
            .interact_with_cancel(pending_snipe)
            .await;

        match loader {
            Ok(Some(Ok(result))) => {
                let mut active_project_write = app_data.active_project.write().await;
                active_project_write.0 = Some(result.id);
                drop(active_project_write);

                let mut projects_write = app_data.projects.write().await;
                projects_write.insert(result.id, result);
                drop(projects_write);

                return Ok(Some(Menu::ProjectMenu(ProjectMenu)))
            },
            Ok(Some(Err(snipe_err))) => return Err((Menu::Main(MainMenu), AppError::from(snipe_err))),
            Ok(None) => return Ok(Some(Menu::CancelSnipe(CancelSnipe { deployer }))),
            Err(err) => return Err((Menu::CancelSnipe(CancelSnipe { deployer }), AppError::from(err)))
        }
    }
}

pub struct CancelSnipe {
    deployer: Pubkey
}
impl Handler for CancelSnipe {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let rpc_client = app_data.rpc_client.read().await;
        let result = Loader::new()
            .with_prompt("cancel_snipe in progress")
            .interact(rpc_client.cancel_snipe(&self.deployer))
            .await;
        drop(rpc_client);

        if let Err(err) = result {
            return Err((Menu::Main(MainMenu), AppError::from(err)))
        }        

        Ok(Some(Menu::Main(MainMenu)))
    }
}