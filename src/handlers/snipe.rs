use dialoguer::theme::ColorfulTheme;
use dialoguer::Loader;
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
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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
        let snipe_id = pending_snipe.snipe_id.clone();

        let loader = Loader::new()
            .with_prompt("Snipe pending")
            .interact_with_cancel(pending_snipe)
            .await;

        match loader {
            Some(Ok(_result)) => return Ok(Some(Menu::ProjectMenu(ProjectMenu))),
            Some(Err(snipe_err)) => return Err((Menu::Main(MainMenu), AppError::from(snipe_err))),
            None => return Ok(Some(Menu::CancelSnipe(CancelSnipe {
                deployer,
                snipe_id
            })))
        }
    }
}

pub struct CancelSnipe {
    snipe_id: String,
    deployer: Pubkey
}
impl Handler for CancelSnipe {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let rpc_client = app_data.rpc_client.read().await;
        let result = Loader::new()
            .with_prompt("cancel_snipe in progress")
            .interact(rpc_client.cancel_snipe(&self.deployer, &self.snipe_id))
            .await;
        drop(rpc_client);

        if let Err(err) = result {
            return Err((Menu::Main(MainMenu), AppError::from(err)))
        }        

        Ok(Some(Menu::Main(MainMenu)))
    }
}