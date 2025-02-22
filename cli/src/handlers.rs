use colored::Colorize;
use crossterm::event;
use crossterm::event::KeyCode;
use crossterm::style::Color;
use crossterm::style::Stylize;
use moonbois_core::WalletDTO;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use spinoff::{Spinner, spinners};
use tokio::select;
use tokio::time::sleep_until;
use tokio::time::Instant;
use std::sync::Arc;
use std::time::Duration;

use dialoguer::Confirm;
use dialoguer::FuzzySelect;
use dialoguer::Input;
use dialoguer::Select;
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
            Self::Back => format!("{}", "Back".to_string().italic().underline(Color::White).dim())
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

        items.push(format!("{}", ProjectMenuOptions::Buy.to_string().white()));
        items.push(format!("{}", ProjectMenuOptions::Sell.to_string().white()));
        items.push(format!("{}", ProjectMenuOptions::AutoBuy.to_string().white()));
        items.push(format!("{}", ProjectMenuOptions::AutoSell.to_string().white()));
        items.push(ProjectMenuOptions::Delete.to_string());
        items.push(ProjectMenuOptions::Back.to_string());

        let selection = match FuzzySelect::new()
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
            None => return Err((Menu::ProjectMenu(ProjectMenu), AppError::UserNotFound))
        };

        let wallet_count = Input::new()
            .with_prompt("Enter wallet amount")
            .default(5)
            .validate_with(|val: &usize| -> Result<(), String> {
                if val > &sniper_count {
                    return Err("Wallet amount exceeds available wallets".to_string())
                };

                Ok(())
            })
            .interact()
            .unwrap();

        let deployer: Pubkey = Input::new()
            .with_prompt("Enter the deployer address")
            .interact()
            .unwrap();

        let rpc_client = app_data.rpc_client.read().await;
        let snipe_id = rpc_client.create_snipe(deployer, wallet_count).await
            .map_err(|err| {
                (Menu::ProjectMenu(ProjectMenu), AppError::from(err))
            })?;

        let mut spinner = Spinner::new(
            spinners::Moon, 
            format!("{}", "snipe in progress | hit enter to cancel snipe".dim()), 
            None
        );

        let task_snipe_id = snipe_id.clone();
        let cancel = tokio::spawn(async move {
            loop {
                let event_happened = event::poll(Duration::from_millis(200)).unwrap();
                if event_happened {
                    match event::read().unwrap() {
                        event::Event::Key(value) => {
                            if let KeyCode::Enter = value.code {
                                return Ok::<Option<Menu>, (Menu, AppError)>(Some(Menu::CancelSnipe(CancelSnipe {
                                    deployer,
                                    snipe_id: task_snipe_id
                                })))
                            }
                        },
                        _ => ()
                    }
                }
            }
        });
        
        let task_data = Arc::clone(&app_data);
        let polling = tokio::spawn(async move {
            loop {
                let rpc_client = task_data.rpc_client.read().await;
                let snipe_in_progress = match rpc_client.get_snipe_status(&deployer, &snipe_id).await {
                    Ok(result) => result,
                    Err(err) => {
                        return Err((Menu::ProjectMenu(ProjectMenu), AppError::from(err)))
                    }
                };
    
                if !snipe_in_progress {
                    return Ok(Some(Menu::ProjectMenu(ProjectMenu)));
                }
    
                sleep_until(Instant::now() + Duration::from_millis(500)).await;
            }
        });

        select! {
            cancel = cancel => {
                spinner.clear();
                return cancel.unwrap();
            },
            polling = polling => {
                spinner.clear();
                return polling.unwrap();
            }
        }
    }
}

pub struct CancelSnipe {
    snipe_id: String,
    deployer: Pubkey
}
impl Handler for CancelSnipe {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let mut spinner = Spinner::new(spinners::Moon, format!("{}", "cancel_snipe in progress".dim()), None);
        let rpc_client = app_data.rpc_client.read().await;
        rpc_client.cancel_snipe(&self.deployer, &self.snipe_id).await
            .map_err(|err| {
                spinner.clear();

                (Menu::Main(MainMenu), AppError::from(err))
            })?;

        spinner.clear();

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

        selection.push(format!("{}", "Back".to_string().italic().underline(Color::White).dim()));

        let index = Select::new()
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

        let delete = Confirm::new()
            .with_prompt("Are you sure you want to delete this project?")
            .default(false)
            .interact()
            .unwrap();

        if delete {
            let mut spinner = Spinner::new(spinners::Moon, format!("{}", "delete_project in progress".dim()), None);
            let rpc_client = app_data.rpc_client.read().await;
            match rpc_client.delete_project(project_id).await {
                Ok(result) => result,
                Err(err) => {
                    spinner.clear();

                    return Err((Menu::ProjectMenu(ProjectMenu), AppError::from(err)));
                }
            };
        
            let mut project_write = app_data.projects.write().await;
            project_write.remove(&project_id);
            let mut active_project = app_data.active_project.write().await;
            active_project.0 = None;

            spinner.clear();

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
    
                    let mut spinner = Spinner::new(spinners::Moon, format!("{}", "sell in progress".dim()), None);
                    let rpc_client = app_data.rpc_client.read().await;
                    rpc_client.sell(project_id, wallet.id).await
                        .map_err(|err| {
                            spinner.clear();
                            
                            (Menu::ProjectMenu(ProjectMenu), AppError::from(err))
                        })?;
                    
                    spinner.clear();
            }
        } else {
            let project_id = match app_data.active_project.read().await.0 {
                Some(project_id) => project_id,
                None => return Err((Menu::Main(MainMenu), AppError::ProjectNotFound))
            };

            let mut spinner = Spinner::new(spinners::Moon, format!("{}", "sell in progress".dim()), None);
            let rpc_client = app_data.rpc_client.read().await;
            rpc_client.auto_sell(project_id).await
                .map_err(|err| {
                    spinner.clear();
                    
                    (Menu::ProjectMenu(ProjectMenu), AppError::from(err))
                })?;
            
            spinner.clear();
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
    
                    let amount: f64 = Input::new().with_prompt("Enter the SOL amount to buy").interact().unwrap();
                    let amount = amount * LAMPORTS_PER_SOL as f64;
    
                    let mut spinner = Spinner::new(spinners::Moon, format!("{}", "buy in progress".dim()), None);
                    let rpc_client = app_data.rpc_client.read().await;
                    rpc_client.buy(project_id, wallet.id, amount as u64).await
                        .map_err(|err| {
                            spinner.clear();
                            
                            (Menu::ProjectMenu(ProjectMenu), AppError::from(err))
                        })?;
                    
                    spinner.clear();
            }
        } else {
            let project_id = match app_data.active_project.read().await.0 {
                Some(project_id) => project_id,
                None => return Err((Menu::ProjectMenu(ProjectMenu), AppError::ProjectNotFound))
            };

            let amount: f64 = Input::new().with_prompt("Enter the SOL amount to buy").interact().unwrap();
            let amount = amount * LAMPORTS_PER_SOL as f64;

            let mut spinner = Spinner::new(spinners::Moon, format!("{}", "buy in progress".dim()), None);
            let rpc_client = app_data.rpc_client.read().await;
            rpc_client.auto_buy(project_id, amount as u64).await
                .map_err(|err| {
                    spinner.clear();
                    
                    (Menu::ProjectMenu(ProjectMenu), AppError::from(err))
                })?;

            spinner.clear();
        }

        Ok(Some(Menu::ProjectMenu(ProjectMenu)))
    }
}

pub struct CreateProject;
impl Handler for CreateProject {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let mint_id: Pubkey = Input::new()
            .with_prompt("Enter the mint id")
            .interact()
            .unwrap();
    
        let rpc_client = app_data.rpc_client.read().await;
        let project = match rpc_client.create_project(mint_id).await {
            Ok(result) => result,
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
        let private_key: String = Input::new()
            .with_prompt("Enter your private key to login")
            .interact()
            .unwrap();

        let signer = if let Ok(private_key_bytes) = serde_json::from_str::<Vec<u8>>(&private_key) {
            Keypair::from_bytes(&private_key_bytes).unwrap()
        } else {
            Keypair::from_base58_string(&private_key)
        };

        let credentials = Credentials { signer };

        let mut rpc_client = app_data.rpc_client.write().await;
        let login_result = rpc_client.login(&credentials).await;

        if let Err(MoonboisClientError::NotFound) = login_result {
            return Ok(Some(Menu::Signup(Signup { credentials })))
        }

        if let Err(err) = login_result {
            return Err((Menu::Login(self), AppError::from(err)));
        }

        if let Ok(()) = login_result {
            let get_user_reponse = rpc_client.get_user().await;
            if let Err(err) = get_user_reponse {
                return Err((Menu::Login(self), AppError::from(err)));
            } else if let Ok(user) = get_user_reponse {
                let mut user_write = app_data.user.write().await;
                user_write.0 = Some(user);
                return Ok(Some(Menu::Main(MainMenu)))
            }
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
        let create_user = Confirm::new()
            .with_prompt(prompt)
            .default(true)
            .interact()
            .unwrap();

        let new_signer = Keypair::new();

        if create_user {
            let mut rpc_client = app_data.rpc_client.write().await;
            if let Err(err) = rpc_client.create_user(&credentials, &new_signer).await {
                return Err((Menu::Signup(Signup { credentials }), AppError::from(err)));
            }
            
            if let Err(err) = rpc_client.login(&credentials).await {
                return Err((Menu::Signup(Signup { credentials }), AppError::from(err)));
            }

            let get_user_reponse = rpc_client.get_user().await;
            if let Err(err) = get_user_reponse {
                return Err((Menu::Login(Login), AppError::from(err)));
            } else if let Ok(user) = get_user_reponse {
                let mut user_write = app_data.user.write().await;
                user_write.0 = Some(user);
                return Ok(Some(Menu::Main(MainMenu)))
            }

            return Ok(Some(Menu::Main(MainMenu)))
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
            Self::RecoverSOL => "RecoverSOL".to_string(),
            Self::Export => "Export".to_string(),
            Self::Exit => format!("{}", "Exit".to_string().italic().underline(Color::White).dim()),
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
            4 => Self::RecoverSOL,
            5 => Self::Export,
            6 => Self::Exit,
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

        let selection = match FuzzySelect::new().with_prompt("Main menu").default(0).items(&vec![
            MainMenuOptions::Snipe,
            MainMenuOptions::NewProject, 
            MainMenuOptions::LoadProject, 
            MainMenuOptions::Wallets, 
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
            MainMenuOptions::RecoverSOL => return Ok(Some(Menu::RecoverSol(RecoverSol))),
            MainMenuOptions::Export => return Ok(Some(Menu::Export(Export))),
            MainMenuOptions::Exit => return Ok(None)
        }
    }
}

pub struct RecoverSol;
impl Handler for RecoverSol {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let confirm = Confirm::new()
            .with_prompt(format!("{}\nDo you want to continue?", Colorize::yellow("This will send all the SOL in your snipers to fee_payer")))
            .default(false)
            .interact()
            .unwrap();

        if !confirm {
            return Ok(Some(Menu::Main(MainMenu)))
        }
        
        let mut spinner = Spinner::new(
            spinners::Moon, 
            format!("{}", "recover_sol in progress".dim()), 
            None
        );
        let rpc_client = app_data.rpc_client.read().await;
        rpc_client.recover_sol().await
            .map_err(|err| {
                spinner.clear();
                
                (Menu::Main(MainMenu), AppError::from(err))
            })?;

        spinner.clear();

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
                wallet.0.clone(), 
                format!("{} {}", wallet.1.sol_balance as f64 / LAMPORTS_PER_SOL as f64, Colorize::cyan("SOL")), 
                wallet.1.token_balance.map(|val| format!("{} {}", (val as f64) / 10f64.powf(6f64), "TOKENS".bright_magenta())).unwrap_or("".to_string())
            ));
            wallets.push(wallet.1.clone());
        }
    }
    drop(user);

    selection.push(format!("{}", "Back".to_string().italic().underline(Color::White).dim()));

    let index = FuzzySelect::new()
        .with_prompt("Select Wallet")
        .default(0)
        .max_length(10)
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
    Back
}

impl ToString for WalletMenuOptions {
    fn to_string(&self) -> String {
        match self {
            Self::Withdraw => "Withdraw".to_string(),
            Self::Deposit => "Deposit".to_string(),
            Self::Send => "Send".to_string(),
            Self::Back => format!("{}", "Back".to_string().italic().underline(Color::White).dim()),
        }
    }
}

impl From<usize> for WalletMenuOptions {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::Withdraw,
            1 => Self::Deposit,
            2 => Self::Send,
            3 => Self::Back,
            _ => panic!("Received invalid main menu index")
        }
    }
}

pub struct WalletMenu;
impl Handler for WalletMenu {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        if let Some(wallet) = select_wallet(app_data).await
            .map_err(|err| (Menu::Main(MainMenu), AppError::from(err)))? {
                let selection = match FuzzySelect::new().with_prompt("Wallet menu").default(0).items(&vec![
                    WalletMenuOptions::Withdraw,
                    WalletMenuOptions::Deposit,
                    WalletMenuOptions::Send,
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
        let amount: f64 = Input::new().with_prompt("Enter the SOL amount").interact().unwrap();
        let amount = amount * LAMPORTS_PER_SOL as f64;
        let mut spinner = Spinner::new(spinners::Moon, format!("{}", "withdraw in progress".dim()), None);
        let receiver = match &app_data.user.read().await.0 {
            Some(user) => user.public_key.clone(),
            None => {
                spinner.clear();

                return Err((Menu::Main(MainMenu), AppError::UserNotFound))
            }
        };
        let rpc_client = app_data.rpc_client.read().await;
        rpc_client.transfer_sol_from_sniper(self.wallet.id, receiver, amount as u64).await
            .map_err(|err| {
                spinner.clear();
                
                (Menu::Main(MainMenu), AppError::from(err))
            })?;

        spinner.clear();

        Ok(Some(Menu::Main(MainMenu)))
    }
}

pub struct Deposit {
    wallet: WalletDTO
}
impl Handler for Deposit {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let amount: f64 = Input::new().with_prompt("Enter the SOL amount").interact().unwrap();
        let amount = amount * LAMPORTS_PER_SOL as f64;
        let mut spinner = Spinner::new(spinners::Moon, format!("{}", "deposit in progress".dim()), None);
        let rpc_client = app_data.rpc_client.read().await;
        rpc_client.transfer_sol_from_main(self.wallet.public_key, amount as u64).await
            .map_err(|err| {
                spinner.clear();

                (Menu::Main(MainMenu), AppError::from(err))
            })?;

        spinner.clear();

        Ok(Some(Menu::Main(MainMenu)))
    }
}

pub struct SendSOL {
    wallet: WalletDTO
}
impl Handler for SendSOL {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let receiver: Pubkey = Input::new().with_prompt("Enter the receiver address").interact().unwrap();
        let amount: f64 = Input::new().with_prompt("Enter the SOL amount").interact().unwrap();
        let amount = amount * LAMPORTS_PER_SOL as f64;
        let mut spinner = Spinner::new(spinners::Moon, format!("{}", "send in progress".dim()), None);
        let rpc_client = app_data.rpc_client.read().await;
        rpc_client.transfer_sol_from_sniper(self.wallet.id, receiver, amount as u64).await
            .map_err(|err| {
                spinner.clear();

                (Menu::Main(MainMenu), AppError::from(err))
            })?;

        spinner.clear();

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

        println!("{:#?}", export);
        FuzzySelect::new()
            .item("Back")
            .default(0)
            .interact()
            .unwrap();

        Ok(Some(Menu::Main(MainMenu)))
    }
}