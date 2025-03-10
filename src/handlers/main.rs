use dialoguer::theme::ColorfulTheme;
use std::sync::Arc;

use dialoguer::FuzzySelect;

use crate::AppData;
use crate::AppError;
use crate::Menu;

use super::project::CreateProject;
use super::project::SelectProject;
use super::snipe::CreateSnipe;
use super::wallet::ImportWallet;
use super::wallet::RecoverSol;
use super::wallet::WalletMenu;
use super::Handler;

pub struct MainMenu;
pub enum MainMenuOptions {
    Snipe,
    NewProject,
    LoadProject,
    Wallets,
    RecoverSOL,
    ImportWallet,
    Export,
    Exit
}

impl ToString for MainMenuOptions {
    fn to_string(&self) -> String {
        match self {
            Self::Snipe => "Snipe".to_string(),
            Self::NewProject => "ImportToken".to_string(),
            Self::LoadProject => "Tokens".to_string(),
            Self::Wallets => "Wallets".to_string(),
            Self::ImportWallet => "ImportWallet".to_string(),
            Self::RecoverSOL => "RecoverSOL".to_string(),
            Self::Export => "Export".to_string(),
            Self::Exit => format!("{}", "Exit"),
        }
    }
}

impl From<usize> for MainMenuOptions {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::Snipe,
            1 => Self::NewProject,
            2 => Self::LoadProject,
            3 => Self::Wallets,
            4 => Self::ImportWallet,
            5 => Self::RecoverSOL,
            6 => Self::Export,
            7 => Self::Exit,
            _ => panic!("Received invalid main menu index")
        }
    }
}

impl Handler for MainMenu {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let mut active_project = app_data.active_project.write().await;
        if active_project.0.is_some() {
            active_project.0 = None;
            drop(active_project);
            return Ok(Some(Menu::Main(MainMenu)));
        }
        drop(active_project);

        let mut user = app_data.user.write().await;
        if let Some(user) = &mut user.0 {
            for (_, wallet) in user.wallets.iter_mut() {
                wallet.token_balance = None;
            }
        }
        drop(user);

        let selection = match FuzzySelect::with_theme(&ColorfulTheme::default()).with_prompt("Main menu").default(0).items(vec![
            MainMenuOptions::Snipe,
            MainMenuOptions::NewProject, 
            MainMenuOptions::LoadProject, 
            MainMenuOptions::Wallets, 
            MainMenuOptions::ImportWallet,
            MainMenuOptions::RecoverSOL, 
            MainMenuOptions::Export, 
            MainMenuOptions::Exit
        ]).interact() {
            Ok(selection) => selection,
            Err(err) => {
                return Err((Menu::Main(MainMenu), AppError::from(err)));
            }
        };

        match MainMenuOptions::from(selection) {
            MainMenuOptions::Snipe => return Ok(Some(Menu::CreateSnipe(CreateSnipe))),
            MainMenuOptions::NewProject => return Ok(Some(Menu::CreateProject(CreateProject))),
            MainMenuOptions::LoadProject => return Ok(Some(Menu::SelectProject(SelectProject))),
            MainMenuOptions::Wallets => return Ok(Some(Menu::Wallet(WalletMenu))),
            MainMenuOptions::ImportWallet => return Ok(Some(Menu::ImportWallet(ImportWallet))),
            MainMenuOptions::RecoverSOL => return Ok(Some(Menu::RecoverSol(RecoverSol))),
            MainMenuOptions::Export => return Ok(Some(Menu::Export(Export))),
            MainMenuOptions::Exit => return Ok(None)
        }
    }
}

pub struct Export;
impl Handler for Export {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let rpc_client = app_data.rpc_client.read().await;
        let export = rpc_client.export().await.map_err(|err| {
                (Menu::Main(MainMenu), AppError::from(err))
            })?;
        drop(rpc_client);

        println!("{:#?}", export);
        FuzzySelect::with_theme(&ColorfulTheme::default())
            .item("Back")
            .default(0)
            .interact()
            .unwrap();

        Ok(Some(Menu::Main(MainMenu)))
    }
}