use console::style;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Loader;
use moonbois_core::WalletDTO;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use std::sync::Arc;

use dialoguer::Confirm;
use dialoguer::FuzzySelect;
use dialoguer::Input;
use moonbois_core::rpc::MoonboisClientError;
use moonbois_core::Credentials;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

use crate::AppData;
use crate::AppError;
use crate::Menu;

pub trait Handler {
    fn handle(self, app_data: &Arc<AppData>) -> impl std::future::Future<Output = Result<Option<Menu>, (Menu, AppError)>> + Send;
}

pub enum ProjectMenuOptions {
    Buy,
    Sell,
    AutoBuy,
    AutoSell,
    Delete,
    Back
}

impl ToString for ProjectMenuOptions {
    fn to_string(&self) -> String {
        match self {
            Self::Buy => "Buy".to_string(),
            Self::Sell => "Sell".to_string(),
            Self::AutoBuy => "AutoBuy".to_string(),
            Self::AutoSell => "AutoSell".to_string(),
            Self::Delete => "Delete".to_string(),
            Self::Back => format!("{}", "Back")
        }
    }
}

impl From<usize> for ProjectMenuOptions {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::Buy,
            1 => Self::Sell,
            2 => Self::AutoBuy,
            3 => Self::AutoSell,
            4 => Self::Delete,
            5 => Self::Back,
            _ => panic!("Received invalid project menu index")
        }
    }
}

pub struct ProjectMenu;
impl Handler for ProjectMenu {
    async fn handle(self, _app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let mut items = vec![];

        items.push(format!("{}", ProjectMenuOptions::Buy.to_string()));
        items.push(format!("{}", ProjectMenuOptions::Sell.to_string()));
        items.push(format!("{}", ProjectMenuOptions::AutoBuy.to_string()));
        items.push(format!("{}", ProjectMenuOptions::AutoSell.to_string()));
        items.push(ProjectMenuOptions::Delete.to_string());
        items.push(ProjectMenuOptions::Back.to_string());

        let selection = match FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Project menu")
            .default(0)
            .items(&items)
            .interact() {
                Ok(selection) => selection,
                Err(err) => {
                    return Err((Menu::Main(MainMenu), AppError::from(err)));
                }
            };

        match ProjectMenuOptions::from(selection) {
            ProjectMenuOptions::Buy => return Ok(Some(Menu::Buy(Buy { auto: false }))),
            ProjectMenuOptions::Sell => return Ok(Some(Menu::Sell(Sell { auto: false }))),
            ProjectMenuOptions::AutoBuy => return Ok(Some(Menu::Buy(Buy { auto: true }))),
            ProjectMenuOptions::AutoSell => return Ok(Some(Menu::Sell(Sell { auto: true }))),
            ProjectMenuOptions::Delete => return Ok(Some(Menu::DeleteProject(DeleteProject))),
            ProjectMenuOptions::Back => return Ok(Some(Menu::Main(MainMenu))),
        };
    }
}

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

pub struct ImportWallet;
impl Handler for ImportWallet {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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

pub struct SelectProject;
impl Handler for SelectProject {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let projects = app_data.projects.read().await;
        let (project_ids, mut selection): (Vec<i32>, Vec<String>) = projects.iter().map(|(key, value)| {
            (key, value.name.clone())
        }).collect();

        selection.push(format!("{}", "Back"));

        let index = FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select Project")
            .default(0)
            .max_length(10)
            .items(&selection)
            .interact()
            .unwrap();

        if index == selection.len() - 1 {
            return Ok(Some(Menu::Main(MainMenu)))
        }

        let mut active_project = app_data.active_project.write().await;
        active_project.0 = Some(project_ids[index]);

        Ok(Some(Menu::ProjectMenu(ProjectMenu)))
    }
}

pub struct DeleteProject;
impl Handler for DeleteProject {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let project_id = match app_data.active_project.read().await.0 {
            Some(project_id) => project_id,
            None => return Err((Menu::ProjectMenu(ProjectMenu), AppError::ProjectNotFound))
        };

        let delete = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Are you sure you want to delete this project?")
            .default(false)
            .interact()
            .unwrap();

        if delete {
            let rpc_client = app_data.rpc_client.read().await;
            let result = Loader::new()
                .with_prompt("delete_project in progress")
                .interact(rpc_client.delete_project(project_id))
                .await;
            drop(rpc_client);
    
            if let Err(err) = result {
                return Err((Menu::Main(MainMenu), AppError::from(err)))
            }
        
            let mut project_write = app_data.projects.write().await;
            project_write.remove(&project_id);
            let mut active_project = app_data.active_project.write().await;
            active_project.0 = None;

            return Ok(Some(Menu::Main(MainMenu)))
        }

        Ok(Some(Menu::ProjectMenu(ProjectMenu)))
    }
}

pub struct Sell {
    auto: bool
}
impl Handler for Sell {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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
impl Handler for Buy {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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

pub struct CreateProject;
impl Handler for CreateProject {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let mint_id: Pubkey = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter contract address")
            .interact_text()
            .unwrap() {
                Some(result) => result,
                None => return Ok(Some(Menu::Main(MainMenu)))
            };
    
        let rpc_client = app_data.rpc_client.read().await;
        let result = Loader::new()
            .with_prompt("import_token in progress")
            .interact(rpc_client.create_project(mint_id))
            .await;
        drop(rpc_client);

        let project = match result {
            Ok(project) => project,
            Err(err) => return Err((Menu::Main(MainMenu), AppError::from(err)))
        };
    
        let mut project_write = app_data.projects.write().await;
        project_write.insert(project.id, project.clone());
        let mut active_project = app_data.active_project.write().await;
        active_project.0 = Some(project.id);
        
        Ok(Some(Menu::ProjectMenu(ProjectMenu)))
    }
}

pub struct Login;
impl Handler for Login {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let private_key: String = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter your private key to login")
            .interact_text()
            .unwrap() {
                Some(value) => value,
                None => return Ok(None)
            };

        let signer = if let Ok(private_key_bytes) = serde_json::from_str::<Vec<u8>>(&private_key) {
            Keypair::from_bytes(&private_key_bytes).unwrap()
        } else {
            Keypair::from_base58_string(&private_key)
        };

        let credentials = Credentials { signer };

        let mut rpc_client = app_data.rpc_client.write().await;
        let login_result = Loader::new()
            .with_prompt("logging in")
            .interact(rpc_client.login(&credentials))
            .await;
        drop(rpc_client);
        
        if let Err(err) = login_result {
            if let MoonboisClientError::NotFound = err {
                return Ok(Some(Menu::Signup(Signup { credentials })))
            }

            return Err((Menu::Login(self), AppError::from(err)));
        }

        let rpc_client = app_data.rpc_client.write().await;
        let get_user_reponse = Loader::new()
            .with_prompt("loading user")
            .interact(rpc_client.get_user())
            .await;
        drop(rpc_client);
    
        if let Err(err) = get_user_reponse {
            return Err((Menu::Login(self), AppError::from(err)));
        } else if let Ok(user) = get_user_reponse {
            let mut user_write = app_data.user.write().await;
            user_write.0 = Some(user);
            return Ok(Some(Menu::Main(MainMenu)))
        }

        return Err((Menu::Login(self), AppError::Unhandled("Unhandled error".to_string())));
    }
}

pub struct Signup {
    credentials: Credentials
}
impl Handler for Signup {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let user = self.credentials.signer.pubkey();
        let credentials = self.credentials;
        let prompt = format!("Unable to find account for {user} would you like to create one?");
        let create_user = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(true)
            .interact()
            .unwrap();

        let new_signer = Keypair::new();

        if create_user {
            let mut rpc_client = app_data.rpc_client.write().await;
            if let Err(err) = Loader::new()
                .with_prompt("creating user")
                .interact(rpc_client.create_user(&credentials, &new_signer))
                .await {
                    return Err((Menu::Signup(Signup { credentials }), AppError::from(err)));
                }

            if let Err(err) = Loader::new()
                .with_prompt("logging in")
                .interact(rpc_client.login(&credentials))
                .await {
                    return Err((Menu::Signup(Signup { credentials }), AppError::from(err)));
                }

            let get_user_reponse = Loader::new()
                .with_prompt("loading user")
                .interact(rpc_client.get_user())
                .await;
            drop(rpc_client);

            match get_user_reponse {
                Ok(user) => {
                    let mut user_write = app_data.user.write().await;
                    user_write.0 = Some(user);
                    return Ok(Some(Menu::Main(MainMenu)))
                },
                Err(err) => return Err((Menu::Login(Login), AppError::from(err)))
            }
        }

        return Ok(Some(Menu::Login(Login)))
    }
}

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
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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

pub struct RecoverSol;
impl Handler for RecoverSol {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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

async fn select_wallet(app_data: &Arc<AppData>) -> Result<Option<WalletDTO>, AppError> {
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
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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

pub struct Withdraw {
    wallet: WalletDTO
}
impl Handler for Withdraw {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {        
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
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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

pub struct Export;
impl Handler for Export {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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