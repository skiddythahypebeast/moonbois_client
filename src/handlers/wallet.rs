use console::style;
use dialoguer::theme::ColorfulTheme;
use crate::dialogue::loader::Loader;
use moonbois_core::WalletDTO;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use std::sync::Arc;

use dialoguer::Confirm;
use dialoguer::FuzzySelect;
use dialoguer::Input;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;

use crate::AppData;
use crate::AppError;
use crate::Menu;

use super::main::MainMenu;
use super::Handler;

pub enum WalletMenuOptions {
    Withdraw,
    Deposit,
    Send,
    Delete,
    Back
}

impl ToString for WalletMenuOptions {
    fn to_string(&self) -> String {
        match self {
            Self::Withdraw => "Withdraw".to_string(),
            Self::Deposit => "Deposit".to_string(),
            Self::Send => "Send".to_string(),
            Self::Delete => "Delete".to_string(),
            Self::Back => format!("{}", "Back"),
        }
    }
}

impl From<usize> for WalletMenuOptions {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::Withdraw,
            1 => Self::Deposit,
            2 => Self::Send,
            3 => Self::Delete,
            4 => Self::Back,
            _ => panic!("Received invalid main menu index")
        }
    }
}

pub struct WalletMenu;
impl Handler for WalletMenu {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        if let Some(wallet) = select_wallet(app_data).await
            .map_err(|err| (Menu::Main(MainMenu), AppError::from(err)))? {
                let selection = match FuzzySelect::with_theme(&ColorfulTheme::default()).with_prompt("Wallet menu").default(0).items(vec![
                    WalletMenuOptions::Withdraw,
                    WalletMenuOptions::Deposit,
                    WalletMenuOptions::Send,
                    WalletMenuOptions::Delete,
                    WalletMenuOptions::Back
                ]).interact() {
                    Ok(selection) => selection,
                    Err(err) => {
                        return Err((Menu::Main(MainMenu), AppError::from(err)));
                    }
                };

                match WalletMenuOptions::from(selection) {
                    WalletMenuOptions::Withdraw => return Ok(Some(Menu::Withdraw(Withdraw { wallet }))),
                    WalletMenuOptions::Deposit => return Ok(Some(Menu::Deposit(Deposit { wallet }))),
                    WalletMenuOptions::Send => return Ok(Some(Menu::Send(SendSOL { wallet }))),
                    WalletMenuOptions::Delete => return Ok(Some(Menu::DeleteWallet(DeleteWallet { wallet }))),
                    WalletMenuOptions::Back => return Ok(Some(Menu::Main(MainMenu)))
                }
        }

        Ok(Some(Menu::Main(MainMenu)))
    }
}

pub struct ImportWallet;
impl Handler for ImportWallet {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let private_key: String = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter the private key to import")
            .interact_text()
            .unwrap() {
                Some(value) => value,
                None => return Ok(Some(Menu::Main(MainMenu)))
            };

        let signer = if let Ok(private_key_bytes) = serde_json::from_str::<Vec<u8>>(&private_key) {
            Keypair::from_bytes(&private_key_bytes).unwrap()
        } else {
            Keypair::from_base58_string(&private_key)
        };
        
        let rpc_client = app_data.rpc_client.read().await;
        let result = match Loader::new()
            .with_prompt("import_wallet in progress")
            .interact(rpc_client.import_user_wallet(&signer))
            .await {
                Ok(result) => result,
                Err(err) => return Err((Menu::Main(MainMenu), AppError::from(err)))
            };
        drop(rpc_client);

        let mut user = app_data.user.write().await;
        if let Some(ref mut user) = user.0 {
            user.wallets.insert(result.public_key.to_string(), result);
        }

        Ok(Some(Menu::Main(MainMenu)))
    }
}

pub struct DeleteWallet {
    pub wallet: WalletDTO 
}
impl Handler for DeleteWallet {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let delete = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Are you sure you want to delete this wallet?")
            .default(false)
            .interact()
            .unwrap();

        if !delete {
            return Ok(Some(Menu::Wallet(WalletMenu)));
        }

        let rpc_client = app_data.rpc_client.read().await;
        let result = Loader::new()
            .with_prompt("delete_wallet in progress")
            .interact(rpc_client.delete_user_wallet(self.wallet.id))
            .await;
        drop(rpc_client);

        if let Err(err) = result {
            return Err((Menu::Wallet(WalletMenu), AppError::from(err)))
        }

        let mut user = app_data.user.write().await;
        if let Some(ref mut user) = user.0 {
            user.wallets.remove(&self.wallet.public_key.to_string());
        }

        Ok(Some(Menu::Main(MainMenu)))
    }
}

pub struct RecoverSol;
impl Handler for RecoverSol {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("{}\nDo you want to continue?", style("This will send all the SOL in your snipers to fee_payer").yellow()))
            .default(false)
            .interact()
            .unwrap();

        if !confirm {
            return Ok(Some(Menu::Main(MainMenu)))
        }
        
        let rpc_client = app_data.rpc_client.read().await;
        let result = Loader::new()
            .with_prompt("recover_sol in progress")
            .interact(rpc_client.recover_sol())
            .await;
        drop(rpc_client);

        if let Err(err) = result {
            return Err((Menu::Main(MainMenu), AppError::from(err)))
        }

        Ok(Some(Menu::Main(MainMenu)))
    }
}

pub async fn select_wallet(app_data: &Arc<AppData>) -> Result<Option<WalletDTO>, AppError> {
    let user = app_data.user.read().await;
    let mut selection: Vec<String> = vec![];
    let mut wallets: Vec<WalletDTO> = vec![];
    if let Some(user) = &user.0 {
        for wallet in user.wallets.iter() {
            selection.push(format!(
                "{} {} {}",
                wallet.0.clone()[0..5].to_string(), 
                format!("{} {}", wallet.1.sol_balance as f64 / LAMPORTS_PER_SOL as f64, "SOL"), 
                wallet.1.token_balance.map(|val| format!("{} {}", (val as f64) / 10f64.powf(6f64), "TOKENS")).unwrap_or("".to_string())
            ));
            wallets.push(wallet.1.clone());
        }
    }
    drop(user);

    selection.push(format!("{}", "Back"));

    let index = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select Wallet")
        .default(0)
        .items(&selection)
        .interact()
        .unwrap();

    if index == selection.len() - 1 {
        return Ok(None)
    }

    Ok(wallets.get(index).cloned())
}

pub struct Withdraw {
    wallet: WalletDTO
}
impl Handler for Withdraw {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let amount: f64 = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter the SOL amount")
            .interact_text()
            .unwrap() {
                Some(value) => value,
                None => return Ok(Some(Menu::Wallet(WalletMenu)))
            };

        let amount = amount * LAMPORTS_PER_SOL as f64;
        
        let receiver = match &app_data.user.read().await.0 {
            Some(user) => user.public_key.clone(),
            None => return Err((Menu::Main(MainMenu), AppError::UserNotFound))
        };

        let rpc_client = app_data.rpc_client.read().await;
        let result = Loader::new()
            .with_prompt("withdraw in progress")
            .interact( rpc_client.transfer_sol_from_sniper(self.wallet.id, receiver, amount as u64))
            .await;
        drop(rpc_client);

        if let Err(err) = result {
            return Err((Menu::Main(MainMenu), AppError::from(err)))
        }

        Ok(Some(Menu::Main(MainMenu)))
    }
}

pub struct Deposit {
    wallet: WalletDTO
}
impl Handler for Deposit {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {        
        let amount: f64 = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter the SOL amount")
            .interact_text()
            .unwrap() {
                Some(value) => value,
                None => return Ok(Some(Menu::Wallet(WalletMenu)))
            };

        let amount = amount * LAMPORTS_PER_SOL as f64;
        let rpc_client = app_data.rpc_client.read().await;
        let result = Loader::new()
            .with_prompt("deposit in progress")
            .interact(rpc_client.transfer_sol_from_main(self.wallet.public_key, amount as u64))
            .await;
        drop(rpc_client);

        if let Err(err) = result {
            return Err((Menu::Main(MainMenu), AppError::from(err)))
        }

        Ok(Some(Menu::Main(MainMenu)))
    }
}

pub struct SendSOL {
    wallet: WalletDTO
}
impl Handler for SendSOL {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let receiver: Pubkey = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter the receiver address")
            .interact_text()
            .unwrap() {
                Some(value) => value,
                None => return Ok(Some(Menu::Wallet(WalletMenu)))
            };

        let amount: f64 = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter the SOL amount")
            .interact_text()
            .unwrap() {
                Some(value) => value,
                None => return Ok(Some(Menu::Wallet(WalletMenu)))
            };
        let amount = amount * LAMPORTS_PER_SOL as f64;

        let rpc_client = app_data.rpc_client.read().await;
        let result = Loader::new()
            .with_prompt("send in progress")
            .interact(rpc_client.transfer_sol_from_sniper(self.wallet.id, receiver, amount as u64))
            .await;
        drop(rpc_client);

        if let Err(err) = result {
            return Err((Menu::Main(MainMenu), AppError::from(err)))
        }

        Ok(Some(Menu::Main(MainMenu)))
    }
}