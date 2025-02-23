use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use console::style;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use handlers::Buy;
use handlers::CancelSnipe;
use handlers::CreateProject;
use handlers::CreateSnipe;
use handlers::DeleteProject;
use handlers::DeleteWallet;
use handlers::Deposit;
use handlers::Export;
use handlers::Handler;
use handlers::ImportWallet;
use handlers::RecoverSol;
use handlers::SelectProject;
use handlers::Login;
use handlers::MainMenu;
use handlers::ProjectMenu;
use handlers::Sell;
use handlers::SendSOL;
use handlers::Signup;
use handlers::WalletMenu;
use handlers::Withdraw;
use moonbois_core::rpc::MoonboisClient;
use moonbois_core::rpc::MoonboisClientError;
use moonbois_core::ProjectDTO;
use moonbois_core::UserDTO;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::pubkey::ParsePubkeyError;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::sleep_until;
use tokio::time::Instant;

pub mod handlers;

static BANNER: &str = r#"
 _____ _____ _____ _____ _____ _____ _____ _____ 
|     |     |     |   | | __  |     |     |   __|
| | | |  |  |  |  | | | | __ -|  |  |-   -|__   |
|_|_|_|_____|_____|_|___|_____|_____|_____|_____|
"#;

pub struct ActiveProject(pub Option<i32>);
pub struct ActiveUser(pub Option<UserDTO>);

pub struct AppData {
    pub rpc_client: RwLock<MoonboisClient>,
    pub user: RwLock<ActiveUser>,
    pub projects: RwLock<HashMap<i32, ProjectDTO>>,
    pub active_project: RwLock<ActiveProject>
}

pub enum Menu {
    Main(MainMenu),
    Login(Login),
    Signup(Signup),
    Wallet(WalletMenu),
    Send(SendSOL),
    ImportWallet(ImportWallet),
    DeleteWallet(DeleteWallet),
    Buy(Buy),
    DeleteProject(DeleteProject),
    CancelSnipe(CancelSnipe),
    CreateSnipe(CreateSnipe),
    Sell(Sell),
    Withdraw(Withdraw),
    Deposit(Deposit),
    ProjectMenu(ProjectMenu),
    CreateProject(CreateProject),
    SelectProject(SelectProject),
    RecoverSol(RecoverSol),
    Export(Export)
}
impl Handler for Menu {
    async fn handle(self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        match self {
            Menu::Main(handler) => handler.handle(app_data).await,
            Menu::Login(handler) => handler.handle(app_data).await,
            Menu::Signup(handler) => handler.handle(app_data).await,
            Menu::CreateProject(handler) => handler.handle(app_data).await,
            Menu::Wallet(handler) => handler.handle(app_data).await,
            Menu::ProjectMenu(handler) => handler.handle(app_data).await,
            Menu::SelectProject(handler) => handler.handle(app_data).await,
            Menu::Buy(handler) => handler.handle(app_data).await, 
            Menu::DeleteWallet(handler) => handler.handle(app_data).await, 
            Menu::ImportWallet(handler) => handler.handle(app_data).await, 
            Menu::RecoverSol(handler) => handler.handle(app_data).await,
            Menu::CancelSnipe(handler) => handler.handle(app_data).await,
            Menu::CreateSnipe(handler) => handler.handle(app_data).await,
            Menu::DeleteProject(handler) => handler.handle(app_data).await,
            Menu::Sell(handler) => handler.handle(app_data).await,
            Menu::Export(handler) => handler.handle(app_data).await,
            Menu::Send(handler) => handler.handle(app_data).await,
            Menu::Deposit(handler) => handler.handle(app_data).await,
            Menu::Withdraw(handler) => handler.handle(app_data).await,
        }
    }
}

pub struct App {
    app_data: Arc<AppData>,
    socket_handle: JoinHandle<Result<(), String>>
}
impl App {
    pub fn new(app_data: Arc<AppData>) -> Self {
        let app_data_arc = Arc::clone(&app_data);
        Self {
            app_data,
            socket_handle: tokio::spawn(async move {
                loop {
                    let rpc_client = app_data_arc.rpc_client.read().await;
                    if rpc_client.jwt.is_some() { drop(rpc_client); break; }
                    sleep_until(Instant::now() + Duration::from_millis(500)).await;
                }
                
                loop {
                    let rpc_client = app_data_arc.rpc_client.read().await;
                    let projects = match rpc_client.get_user_projects().await {
                        Ok(projects) => projects,
                        Err(err) => return Err(err.to_string())
                    };
                    let mint_id = if let Some(project_id) = &app_data_arc.active_project.read().await.0 {
                        if let Some(project) = projects.get(project_id) {
                            Some(project.pumpfun.mint_id)
                        } else { None }
                    } else { None };

                    let balances = match rpc_client.get_user_balances(mint_id).await {
                        Ok(user) => user,
                        Err(err) => return Err(err.to_string())
                    };

                    drop(rpc_client);
                    
                    let mut project_data = app_data_arc.projects.write().await;
                    project_data.clear();
                    
                    for (id, project) in projects {
                        project_data.insert(id, project);
                    }

                    drop(project_data);

                    if let Some(user_data) = &mut app_data_arc.user.write().await.0 {
                        user_data.sol_balance = balances.user.sol_balance;
                        for balance in balances.wallets {
                            if let Some(wallet) = user_data.wallets.get_mut(&balance.0) {
                                wallet.sol_balance = balance.1.sol_balance;
                                wallet.token_balance = balance.1.token_balance;
                            }
                        }
                    }

                    sleep_until(Instant::now() + Duration::from_millis(500)).await;
                }
            })
        }
    }
    pub async fn run(self, menu: Menu) {
        std::process::Command::new("clear").status().unwrap();
        println!("{}", style(BANNER).bold());
        
        if let Some(user) = &self.app_data.user.read().await.0 {
            let sniper_balance = user.wallets.iter().map(|(_, y)| y.sol_balance).reduce(|x, y| x + y).unwrap() as f64 / LAMPORTS_PER_SOL as f64;
            let user_balance = user.sol_balance as f64 / LAMPORTS_PER_SOL as f64;
            println!(
                "fee_payer: {}\nfee_payer_balance: {}\nsniper_sol_balance: {}",
                user.public_key, 
                format!("{} {}", user_balance, style("SOL").cyan()),
                format!("{} {}", sniper_balance, style("SOL").cyan()),
            );
            if let Some(active_project) = &self.app_data.active_project.read().await.0 {
                if let Some(project) = &self.app_data.projects.read().await.get(active_project) {
                    let sniper_token_balance = user.wallets.iter().filter_map(|(_, x)| x.token_balance).reduce(|a, b| a + b).unwrap_or(0) as f64 / 10f64.powf(6f64);
                    println!(
                        "snipe_token_balance: {} {}",
                        format!("{}", sniper_token_balance),
                        format!("{}", style(project.name.to_uppercase()).magenta())
                    )
                }
            }
        }

        if let Some(active_project) = &self.app_data.active_project.read().await.0 {
            if let Some(project) = &self.app_data.projects.read().await.get(active_project) {
                println!(
                    "mint_id: {}\ndeployer: {}",
                    project.pumpfun.mint_id,
                    project.deployer
                )
            }
        }

        println!("");

        if self.socket_handle.is_finished() {
            let error_message = if let Err(err) = self.socket_handle.await.unwrap() {
                err
            } else {
                "".to_string()
            };

            println!("\n{}\n{}", style("Websocket connection failed ⚠️").yellow(), style(error_message).dim());
            Select::with_theme(&ColorfulTheme::default())
                .items(&vec!["Back"])
                .default(0)
                .interact()
                .unwrap();
            
            return;
        }

        let handler_response = menu.handle(&self.app_data).await;
        if let Err((menu, err)) = handler_response {
            match err {
                AppError::MoonboisClientError(MoonboisClientError::UnhandledServerError(err)) => {
                    println!("{}\n  - {}", style("Unhandled server error occured ⚠️").yellow(), style(err).dim());
                    Select::with_theme(&ColorfulTheme::default())
                        .items(&vec!["Back"])
                        .default(0)
                        .interact()
                        .unwrap();

                        let _ = Box::pin(App::run(self, menu)).await;
                }
                AppError::MoonboisClientError(MoonboisClientError::NotFound) => {
                    println!("{}", style("Requested resource was not found ⚠️").yellow());
                    Select::with_theme(&ColorfulTheme::default())
                        .items(&vec!["Back"])
                        .default(0)
                        .interact()
                        .unwrap();

                        let _ = Box::pin(App::run(self, menu)).await;
                }
                AppError::MoonboisClientError(MoonboisClientError::InvalidUri(err)) => {
                    println!("{}\n  - {}", style("Invalid URI ⚠️").yellow(), style(err.to_string()).dim());
                    Select::with_theme(&ColorfulTheme::default())
                        .items(&vec!["Back"])
                        .default(0)
                        .interact()
                        .unwrap();

                        let _ = Box::pin(App::run(self, menu)).await;
                }
                AppError::MoonboisClientError(MoonboisClientError::JsonError(err)) => {
                    println!("{}\n  - {}", style("JSON error ⚠️").yellow(), style(err.to_string()).dim());
                    Select::with_theme(&ColorfulTheme::default())
                        .items(&vec!["Back"])
                        .default(0)
                        .interact()
                        .unwrap();

                        let _ = Box::pin(App::run(self, menu)).await;
                }
                AppError::MoonboisClientError(MoonboisClientError::MissingJWT) => {
                    println!("{}", style("Authorization failed ⚠️").yellow());
                    Select::with_theme(&ColorfulTheme::default())
                        .items(&vec!["Back"])
                        .default(0)
                        .interact()
                        .unwrap();

                        let _ = Box::pin(App::run(self, Menu::Login(Login))).await;
                }
                AppError::MoonboisClientError(MoonboisClientError::ParseError(err)) => {
                    println!("{}\n  - {}", style("Parse error occured ⚠️").yellow(), style(err.to_string()).dim());
                    Select::with_theme(&ColorfulTheme::default())
                        .items(&vec!["Back"])
                        .default(0)
                        .interact()
                        .unwrap();

                        let _ = Box::pin(App::run(self, menu)).await;
                }
                AppError::MoonboisClientError(MoonboisClientError::ReqwestError(err)) => {
                    println!("{}\n  - {}", style("An error occured ⚠️").yellow(), err);
                    Select::with_theme(&ColorfulTheme::default())
                        .items(&vec!["Back"])
                        .default(0)
                        .interact()
                        .unwrap();

                        let _ = Box::pin(App::run(self, menu)).await;
                }
                AppError::MoonboisClientError(MoonboisClientError::NotAccepted) => {
                    println!("{}\n  - {}", style("Not accepted ⚠️").yellow(), style(err.to_string()).dim());
                    Select::with_theme(&ColorfulTheme::default())
                        .items(&vec!["Back"])
                        .default(0)
                        .interact()
                        .unwrap();

                        let _ = Box::pin(App::run(self, menu)).await;
                }
                AppError::ParsePubkeyError(err) => {
                    println!("{}\n  - {}", style("Invalid pubkey ⚠️").yellow(), style(err.to_string()).dim());
                    Select::with_theme(&ColorfulTheme::default())
                        .items(&vec!["Back"])
                        .default(0)
                        .interact()
                        .unwrap();

                        let _ = Box::pin(App::run(self, menu)).await;
                }
                AppError::DialogueError(err) => {
                    println!("{}\n  - {}", style("Dialogue error occured ⚠️").yellow(), style(err.to_string()).dim());
                    Select::with_theme(&ColorfulTheme::default())
                        .items(&vec!["Back"])
                        .default(0)
                        .interact()
                        .unwrap();

                        let _ = Box::pin(App::run(self, menu)).await;
                }
                AppError::ProjectNotFound => {
                    println!("{}\n", style("Project not found ⚠️").yellow());
                    Select::with_theme(&ColorfulTheme::default())
                        .items(&vec!["Back"])
                        .default(0)
                        .interact()
                        .unwrap();

                        let _ = Box::pin(App::run(self, menu)).await;
                }
                AppError::Unhandled(err) => {
                    println!("{}\n - {}", style("Unhandled error occured ⚠️").yellow(), err);
                    Select::with_theme(&ColorfulTheme::default())
                        .items(&vec!["Back"])
                        .default(0)
                        .interact()
                        .unwrap();

                        let _ = Box::pin(App::run(self, menu)).await;
                }
                AppError::UserNotFound => {
                    println!("{}\n  - {}", style("Unable to find user ⚠️").yellow(), style(err.to_string()).dim());
                    Select::with_theme(&ColorfulTheme::default())
                        .items(&vec!["Back"])
                        .default(0)
                        .interact()
                        .unwrap();

                        let _ = Box::pin(App::run(self, menu)).await;
                }
            }
        } else if let Ok(Some(result)) = handler_response {
            let _ = Box::pin(App::run(self, result)).await;
        }
    }
}

#[tokio::main]
pub async fn main() {
    let app_data = Arc::new(AppData {
        active_project: RwLock::new(ActiveProject(None)),
        projects: RwLock::new(HashMap::new()),
        rpc_client: RwLock::new(MoonboisClient::new()),
        user: RwLock::new(ActiveUser(None))
    });

    App::new(app_data)
        .run(Menu::Login(Login)).await;
}

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Moonbois client error: {0}")]
    MoonboisClientError(#[from] MoonboisClientError),
    #[error("Dialogue error: {0}")]
    DialogueError(#[from] dialoguer::Error),
    #[error("Parse pubkeyu error: {0}")]
    ParsePubkeyError(#[from] ParsePubkeyError),
    #[error("Project not found")]
    ProjectNotFound,
    #[error("User not found")]
    UserNotFound,
    #[error("Unhandled error: {0}")]
    Unhandled(String)
}