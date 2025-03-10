use dialoguer::theme::ColorfulTheme;
use crate::dialogue::loader::Loader;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use std::sync::Arc;

use dialoguer::Input;

use crate::AppData;
use crate::AppError;
use crate::Menu;

use super::main::MainMenu;
use super::project::ProjectMenu;
use super::wallet::select_wallet;
use super::Handler;

pub struct Sell {
    auto: bool
}
impl Sell {
    pub fn new(auto: bool) -> Self {
        Self {
            auto
        }
    }
}
impl Handler for Sell {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        if !self.auto {
            if let Some(wallet) = select_wallet(app_data).await
                .map_err(|err| (Menu::Main(MainMenu), AppError::from(err)))? {
                    let project_id = match app_data.active_project.read().await.0 {
                        Some(project_id) => project_id,
                        None => return Err((Menu::Main(MainMenu), AppError::ProjectNotFound))
                    };

                    let rpc_client = app_data.rpc_client.read().await;
                    let result = Loader::new()
                        .with_prompt("sell in progress")
                        .interact(rpc_client.sell(project_id, wallet.id))
                        .await;
                    drop(rpc_client);

                    if let Err(err) = result {
                        return Err((Menu::ProjectMenu(ProjectMenu), AppError::from(err)))
                    }
            }
        } else {
            let project_id = match app_data.active_project.read().await.0 {
                Some(project_id) => project_id,
                None => return Err((Menu::Main(MainMenu), AppError::ProjectNotFound))
            };

            let rpc_client = app_data.rpc_client.read().await;
            let result = Loader::new()
                .with_prompt("auto_sell in progress")
                .interact(rpc_client.auto_sell(project_id))
                .await;
            drop(rpc_client);

            if let Err(err) = result {
                return Err((Menu::ProjectMenu(ProjectMenu), AppError::from(err)))
            }
        }

        Ok(Some(Menu::ProjectMenu(ProjectMenu)))
    }
}

pub struct Buy {
    auto: bool
}
impl Buy {
    pub fn new(auto: bool) -> Self {
        Self {
            auto
        }
    }
}
impl Handler for Buy {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        if !self.auto {
            if let Some(wallet) = select_wallet(app_data).await
                .map_err(|err| (Menu::Main(MainMenu), AppError::from(err)))? {
                    let project_id = match app_data.active_project.read().await.0 {
                        Some(project_id) => project_id,
                        None => return Err((Menu::Main(MainMenu), AppError::ProjectNotFound))
                    };
    
                    let amount: f64 = match Input::with_theme(&ColorfulTheme::default())
                        .with_prompt("Enter the SOL amount to buy")
                        .interact_text()
                        .unwrap() {
                            Some(result) => result,
                            None => return Ok(Some(Menu::ProjectMenu(ProjectMenu)))
                        };
                    let amount = amount * LAMPORTS_PER_SOL as f64;
                    
                    let rpc_client = app_data.rpc_client.read().await;
                    let result = Loader::new()
                        .with_prompt("buy in progress")
                        .interact(rpc_client.buy(project_id, wallet.id, amount as u64))
                        .await;
                    drop(rpc_client);

                    if let Err(err) = result {
                        return Err((Menu::ProjectMenu(ProjectMenu), AppError::from(err)))
                    }
            }
        } else {
            let project_id = match app_data.active_project.read().await.0 {
                Some(project_id) => project_id,
                None => return Err((Menu::ProjectMenu(ProjectMenu), AppError::ProjectNotFound))
            };

            let amount: f64 = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter the SOL amount to buy")
            .interact_text()
            .unwrap() {
                Some(result) => result,
                None => return Ok(Some(Menu::ProjectMenu(ProjectMenu)))
            };

            let amount = amount * LAMPORTS_PER_SOL as f64;

            let rpc_client = app_data.rpc_client.read().await;
            let result = Loader::new()
                .with_prompt("buy in progress")
                .interact(rpc_client.auto_buy(project_id, amount as u64))
                .await;
            drop(rpc_client);

            if let Err(err) = result {
                return Err((Menu::ProjectMenu(ProjectMenu), AppError::from(err)))
            }
        }

        Ok(Some(Menu::ProjectMenu(ProjectMenu)))
    }
}