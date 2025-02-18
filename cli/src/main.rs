use std::str::FromStr;
use std::time::Duration;

use colored::Colorize;
use dialoguer::Confirm;
use dialoguer::FuzzySelect;
use dialoguer::Input;
use dialoguer::Select;
use moonbois_core::Credentials;
use moonbois_core::rpc::MoonboisClient;
use moonbois_core::rpc::MoonboisClientError;
use moonbois_core::ProjectDTO;
use moonbois_core::UserDTO;
use moonbois_core::WalletDTO;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use crossterm::event::KeyCode;
use crossterm::event;

pub struct UserStore {
    inner: UserDTO,
}

impl UserStore {
    pub async fn update_balances(&mut self, rpc_client: &MoonboisClient) -> Result<(), MoonboisClientError> {
        let balances = rpc_client.get_user_balances().await?;
        self.inner.sol_balance = balances.user.sol_balance;
        for (public_key, wallet) in self.inner.wallets.iter_mut() {
            if let Some(value) = balances.wallets.get(&public_key) {
                wallet.sol_balance = value.sol_balance;
                wallet.token_balance = value.token_balance;
            }
        }

        Ok(())
    }
}

static BANNER: &str = r#"           
        :::   :::    ::::::::   ::::::::  ::::    ::: :::::::::   :::::::: ::::::::::: :::::::: 
      :+:+: :+:+:  :+:    :+: :+:    :+: :+:+:   :+: :+:    :+: :+:    :+:    :+:    :+:    :+: 
    +:+ +:+:+ +:+ +:+    +:+ +:+    +:+ :+:+:+  +:+ +:+    +:+ +:+    +:+    +:+    +:+         
   +#+  +:+  +#+ +#+    +:+ +#+    +:+ +#+ +:+ +#+ +#++:++#+  +#+    +:+    +#+    +#++:++#++   
  +#+       +#+ +#+    +#+ +#+    +#+ +#+  +#+#+# +#+    +#+ +#+    +#+    +#+           +#+    
 #+#       #+# #+#    #+# #+#    #+# #+#   #+#+# #+#    #+# #+#    #+#    #+#    #+#    #+#     
###       ###  ########   ########  ###    #### #########   ######## ########### ########   
"#;

#[tokio::main]
async fn main() {
    std::process::Command::new("clear").status().unwrap();
    println!("{}\n", BANNER);
    dotenv::dotenv().ok();
    let mut rpc_client = MoonboisClient::new();

    let private_key: String = Input::new()
        .with_prompt("Enter your private key to login")
        .interact()
        .unwrap();

    let signer = if let Ok(private_key_bytes) = serde_json::from_str::<Vec<u8>>(&private_key) {
        Keypair::from_bytes(&private_key_bytes).unwrap()
    } else {
        Keypair::from_base58_string(&private_key)
    };
    let account_address = signer.pubkey();
    let credentials = Credentials { signer };
    
    if let Err(err) = rpc_client.login(&credentials).await {
        match err {
            MoonboisClientError::NotFound => {
                let prompt = format!("Unable to find account for {} would you like to create one?", account_address);
                let create_user = Confirm::new()
                    .with_prompt(prompt)
                    .default(true)
                    .interact()
                    .unwrap();

                let new_signer = Keypair::new();

                if create_user {
                    rpc_client.create_user(&credentials, &new_signer).await.unwrap();
                }

                rpc_client.login(&credentials).await.unwrap();
            }
            _ => panic!("{err}")
        };
    }
    let user = rpc_client.get_user().await.unwrap();
    let mut user_store = UserStore {
        inner: user
    };

    main_menu(&rpc_client, &mut user_store, &credentials).await;
}

async fn main_menu(rpc_client: &MoonboisClient, user_store: &mut UserStore, credentials: &Credentials) {
    loop {
        std::process::Command::new("clear").status().unwrap();
        println!(
            "{BANNER}\nLogged in as: {}\nFunding: {}\nFunding SOL Balance: {}\n Sniper SOL Balance: {}", 
            credentials.signer.pubkey().to_string().on_red(), 
            user_store.inner.public_key,
            user_store.inner.sol_balance as f64 / LAMPORTS_PER_SOL as f64, 
            user_store.inner.wallets.iter().map(|wallet| wallet.1.sol_balance).reduce(|a, b| a + b).unwrap() as f64 / LAMPORTS_PER_SOL as f64, 
        );

        let selections = vec!["New Project", "Load Project", "Wallets", "Recover SOL", "Export", "Exit"];

        match FuzzySelect::new()
            .with_prompt("Main menu")
            .default(0)
            .items(&selections)
            .interact()
            .unwrap() {
                0 => {
                    if let Some(project) = create_project(rpc_client).await {
                        project_menu(project, user_store, rpc_client, credentials).await;
                    }
                },
                1 => {
                    if let Some(project) = load_project(&rpc_client).await {
                        project_menu(project, user_store, rpc_client, credentials).await;
                    }
                },
                2 => {
                    let selected_wallet = wallet_select(user_store).await;
                    if let Some(wallet) = selected_wallet {
                        wallet_menu(&wallet, user_store, credentials, rpc_client).await;
                    }
                }
                3 => {
                    if let Err(err) = rpc_client.recover_sol().await {
                        match err {
                            MoonboisClientError::ServerError(err) => {
                                println!("\n{}\n  - {}", "Failed to recover SOL ⚠️".yellow(), err.dimmed());
                                Select::new()
                                    .items(&vec!["Back"])
                                    .default(0)
                                    .interact()
                                    .unwrap();
                            }
                            _ => panic!("{err}")
                        }
                    }
                }
                4 => {
                    if let Ok(exported_data) = rpc_client.export().await {
                        println!("{:#?}", exported_data);
                        
                        Select::new()
                            .items(&vec!["Back"])
                            .default(0)
                            .interact()
                            .unwrap();
                    };
                }
                5 => break,
                _ => ()
            }
    }

    std::process::Command::new("clear").status().unwrap();
}

async fn wallet_menu(wallet: &WalletDTO, user_store: &mut UserStore, credentials: &Credentials, rpc_client: &MoonboisClient) {
    std::process::Command::new("clear").status().unwrap();
    println!(
        "{BANNER}\nLogged in as: {}\nFunding: {}\nFunding SOL Balance: {}\n Sniper SOL Balance: {}", 
        credentials.signer.pubkey().to_string().on_red(), 
        user_store.inner.public_key,
        user_store.inner.sol_balance as f64 / LAMPORTS_PER_SOL as f64, 
        user_store.inner.wallets.iter().map(|wallet| wallet.1.sol_balance).reduce(|a, b| a + b).unwrap() as f64 / LAMPORTS_PER_SOL as f64, 
    );

    loop {
        user_store.update_balances(rpc_client).await.unwrap();
        std::process::Command::new("clear").status().unwrap();
        println!(
            "{BANNER}\nLogged in as: {}\nFunding: {}\nFunding SOL Balance: {}\n Sniper SOL Balance: {}", 
            credentials.signer.pubkey().to_string().on_red(), 
            user_store.inner.public_key,
            user_store.inner.sol_balance as f64 / LAMPORTS_PER_SOL as f64, 
            user_store.inner.wallets.iter().map(|wallet| wallet.1.sol_balance).reduce(|a, b| a + b).unwrap() as f64 / LAMPORTS_PER_SOL as f64, 
        );

        let selections = vec![
            format!("Deposit {}", "SOL".cyan()), 
            format!("Withdraw {}", "SOL".cyan()), 
            format!("Send {}", "SOL".cyan()), 
            format!("{}", "Back")
        ];

        match FuzzySelect::new()
            .with_prompt(format!("{}", wallet.public_key))
            .default(0)
            .items(&selections)
            .interact()
            .unwrap() {
                0 => {                        
                    let amount_in_sol: f64 = Input::new()
                        .with_prompt("Enter the deposit amount in sol")
                        .interact()
                        .unwrap();

                    let amount = (amount_in_sol * LAMPORTS_PER_SOL as f64) as u64;

                    
                    if let Err(err) = rpc_client.transfer_sol_from_main(wallet.public_key, amount).await {
                        match err {
                            MoonboisClientError::ServerError(err) => {
                                println!("\n{}\n  - {}", "Failed to deposit SOL ⚠️".yellow(), err.dimmed());
                                Select::new()
                                    .items(&vec!["Back"])
                                    .default(0)
                                    .interact()
                                    .unwrap();
                            }
                            _ => panic!("{err}")
                        }
                    };
                },
                1 => {            
                    let amount_in_sol: f64 = Input::new()
                        .with_prompt("Enter the withdrawal amount in sol")
                        .interact()
                        .unwrap();

                    let amount = (amount_in_sol * LAMPORTS_PER_SOL as f64) as u64;

                    if let Err(err) = rpc_client.transfer_sol_from_sniper(wallet.id, user_store.inner.public_key, amount).await {
                        match err {
                            MoonboisClientError::ServerError(err) => {
                                println!("\n{}\n  - {}", "Failed to withdraw SOL ⚠️".yellow(), err.dimmed());
                                Select::new()
                                    .items(&vec!["Back"])
                                    .default(0)
                                    .interact()
                                    .unwrap();
                            }
                            _ => panic!("{err}")
                        }
                    }
                },
                2 => {
                    let amount_in_sol: f64 = Input::new()
                        .with_prompt("Enter the send amount in sol")
                        .interact()
                        .unwrap();

                    let receiver: Pubkey = Input::new()
                        .with_prompt("Enter the receiver address")
                        .interact()
                        .unwrap();

                    let amount = (amount_in_sol * LAMPORTS_PER_SOL as f64) as u64;

                    if let Err(err) = rpc_client.transfer_sol_from_sniper(wallet.id, receiver, amount).await {
                        match err {
                            MoonboisClientError::ServerError(err) => {
                                println!("\n{}\n  - {}", "Failed to send SOL ⚠️".yellow(), err.dimmed());
                                Select::new()
                                    .items(&vec!["Back"])
                                    .default(0)
                                    .interact()
                                    .unwrap();
                            }
                            _ => panic!("{err}")
                        }
                    }
                },
                3 => break,
                _ => ()
            }
        }
}

async fn wallet_select<'a>(user_store: &mut UserStore) -> Option<WalletDTO> {
    let mut selection = vec![];

    for (_, wallet) in user_store.inner.wallets.iter() {
        if let Some(token_balance) = wallet.token_balance {
            selection.push(format!("{} {} SOL {} TOKENS", wallet.public_key, wallet.sol_balance as f64 / LAMPORTS_PER_SOL as f64, token_balance));
        } else {
            selection.push(format!("{} {} SOL", wallet.public_key, wallet.sol_balance as f64 / LAMPORTS_PER_SOL as f64));
        }
    }

    let index = FuzzySelect::new()
        .with_prompt("Wallets")
        .default(0)
        .items(&vec![
            selection, 
            vec!["Back".to_string()]
        ].concat())
        .interact()
        .unwrap();

    if index == user_store.inner.wallets.len() { 
        return None; 
    }

    None
}

async fn project_menu(mut project: ProjectDTO, user_store: &mut UserStore, rpc_client: &MoonboisClient, credentials: &Credentials) {
    std::process::Command::new("clear").status().unwrap();
    if let Some(pumpfun) = &project.pumpfun {
        println!(
            "{BANNER}\nLogged in as: {}\nFunding: {}\nFunding SOL Balance: {}\nProject: {}\nDeployer: {}\nMint ID: {}\n", 
            credentials.signer.pubkey().to_string().on_red(), 
            user_store.inner.public_key,
            user_store.inner.sol_balance as f64 / LAMPORTS_PER_SOL as f64, 
            project.name,
            project.deployer,
            pumpfun.mint_id,
        );
    } else {
        println!(
            "{BANNER}\nLogged in as: {}\nFunding: {}\nFunding SOL Balance: {}\nProject: {}\nDeployer: {}\nMint: not_deployed\n", 
            credentials.signer.pubkey().to_string().on_red(), 
            user_store.inner.public_key,
            user_store.inner.sol_balance as f64 / LAMPORTS_PER_SOL as f64, 
            project.name,
            project.deployer,
        );
    }

    loop {
        user_store.update_balances(rpc_client).await.unwrap();
        // let balance = solana.get_balance(&user.public_key).await.unwrap();
        std::process::Command::new("clear").status().unwrap();
        if let Some(pumpfun) = &project.pumpfun {
            println!(
                "{BANNER}\nLogged in as: {}\nFunding: {}\nFunding SOL Balance: {}\nProject: {}\nDeployer: {}\nMint ID: {}\n", 
                credentials.signer.pubkey().to_string().on_red(), 
                user_store.inner.public_key,
                user_store.inner.sol_balance as f64 / LAMPORTS_PER_SOL as f64, 
                project.name,
                project.deployer,
                pumpfun.mint_id,
            );
        } else {
            println!(
                "{BANNER}\nLogged in as: {}\nFunding: {}\nFunding SOL Balance: {}\nProject: {}\nDeployer: {}\nMint: not_deployed\n", 
                credentials.signer.pubkey().to_string().on_red(), 
                user_store.inner.public_key,
                user_store.inner.sol_balance as f64 / LAMPORTS_PER_SOL as f64, 
                project.name,
                project.deployer,
            );
        }
    
        if rpc_client.get_snipe_status(project.id).await.unwrap() {
            println!("Snipe in progress | Hit enter to cancel\r");
            
            loop {
                if !rpc_client.get_snipe_status(project.id).await.unwrap() {
                    break;
                }
                let event_happened = event::poll(Duration::from_millis(500)).unwrap();
                if event_happened {
                    match event::read().unwrap() {
                        event::Event::Key(value) => {
                            if let KeyCode::Enter = value.code {
                                if let Err(err) = rpc_client.cancel_snipe(project.id).await {
                                    match err {
                                        MoonboisClientError::ServerError(err) => {
                                            println!("\n{}\n  - {}", "Failed to cancel snipe ⚠️".yellow(), err.dimmed());
                                            Select::new()
                                                .items(&vec!["Back"])
                                                .default(0)
                                                .interact()
                                                .unwrap();

                                            // let balance = solana.get_balance(&user.public_key).await.unwrap();
                                            std::process::Command::new("clear").status().unwrap();
                                            if let Some(pumpfun) = &project.pumpfun {
                                                println!(
                                                    "{BANNER}\nLogged in as: {}\nFunding: {}\nFunding SOL Balance: {}\nProject: {}\nDeployer: {}\nMint ID: {}\n", 
                                                    credentials.signer.pubkey().to_string().on_red(), 
                                                    user_store.inner.public_key,
                                                    user_store.inner.sol_balance as f64 / LAMPORTS_PER_SOL as f64, 
                                                    project.name,
                                                    project.deployer,
                                                    pumpfun.mint_id,
                                                );
                                            } else {
                                                println!(
                                                    "{BANNER}\nLogged in as: {}\nFunding: {}\nFunding SOL Balance: {}\nProject: {}\nDeployer: {}\nMint: not_deployed\n", 
                                                    credentials.signer.pubkey().to_string().on_red(), 
                                                    user_store.inner.public_key,
                                                    user_store.inner.sol_balance as f64 / LAMPORTS_PER_SOL as f64, 
                                                    project.name,
                                                    project.deployer,
                                                );
                                            }

                                            println!("Snipe in progress | Hit enter to cancel\r");
                
                                            continue;
                                        }
                                        _ => panic!("{err}")
                                    }
                                }
                            }
                        },
                        _ => ()
                    }
                }
            }

            if let Some(update) = match rpc_client.get_project(project.id).await {
                Ok(project) => Some(project),
                Err(err) => {
                    match err {
                        MoonboisClientError::ServerError(err) => {
                            println!("\n{}\n  - {}", "Failed to update project ⚠️".yellow(), err.dimmed());
                            Select::new()
                                .items(&vec!["Back"])
                                .default(0)
                                .interact()
                                .unwrap();

                            None
                        }
                        _ => panic!("{err}")
                    }
                }
            } {
                project = update;
            }

            continue;
        }

        project.pending_snipe = false;

        let mut selections = if project.pumpfun.is_some() {
            vec!["Buy".white(), "Sell".white(), "Auto buy".white(), "Auto sell".white()]
        } else {
            vec!["Buy".white().dimmed(), "Sell".white().dimmed(), "Auto buy".dimmed(), "Auto sell".dimmed()]
        };

        if project.pending_snipe {
            selections.push("Cancel snipe".white());
        } else {
            if project.pumpfun.is_some() {
                selections.push("Snipe".dimmed().white());
            } else {
                selections.push("Snipe".white());
            }
        }

        selections.push("Refresh".white());
        selections.push("Delete".white());
        selections.push("Back".white());
        
        match FuzzySelect::new()
            .with_prompt("Project Menu")
            .default(0)
            .items(&selections)
            .interact()
            .unwrap() {
                0 => {
                    // let balance = solana.get_balance(&user.public_key).await.unwrap();
                    std::process::Command::new("clear").status().unwrap();
                    println!("{BANNER}\nLogged in as: {}\nFunding SOL Balance: {}\n", 
                        credentials.signer.pubkey().to_string().on_red(), 
                        user_store.inner.sol_balance as f64 / LAMPORTS_PER_SOL as f64,
                    );

                    if let Some(wallet) = wallet_select(user_store).await {
                        let amount_in_sol: f64 = Input::new()
                            .with_prompt("Enter the amount in sol")
                            .interact()
                            .unwrap();
                        
                        if let Err(err) = rpc_client.buy(project.id, wallet.id, (amount_in_sol * LAMPORTS_PER_SOL as f64) as u64).await {
                            match err {
                                MoonboisClientError::ServerError(err) => {
                                    println!("\n{}\n  - {}", "Failed to buy tokens ⚠️".yellow(), err.dimmed());
                                    Select::new()
                                        .items(&vec!["Back"])
                                        .default(0)
                                        .interact()
                                        .unwrap();
                                }
                                _ => panic!("{err}")
                            }
                        }
                    }
                }
                1 => {
                    if let Some(wallet) = wallet_select(user_store).await {
                        if let Err(err) = rpc_client.sell(project.id, wallet.id).await {
                            match err {
                                MoonboisClientError::ServerError(err) => {
                                    println!("\n{}\n  - {}", "Failed to sell tokens ⚠️".yellow(), err.dimmed());
                                    Select::new()
                                        .items(&vec!["Back"])
                                        .default(0)
                                        .interact()
                                        .unwrap();
                                }
                                _ => panic!("{err}")
                            }
                        }
                    }
                }
                2 => {
                    let amount_in_sol: f64 = Input::new()
                        .with_prompt("Enter the amount in sol")
                        .interact()
                        .unwrap();

                    if let Err(err) = rpc_client.auto_buy(project.id, (amount_in_sol * LAMPORTS_PER_SOL as f64) as u64).await {
                        match err {
                            MoonboisClientError::ServerError(err) => {
                                println!("\n{}\n  - {}", "Failed to auto buy tokens ⚠️".yellow(), err.dimmed());
                                Select::new()
                                    .items(&vec!["Back"])
                                    .default(0)
                                    .interact()
                                    .unwrap();
                            }
                            _ => panic!("{err}")
                        }
                    }
                }
                3 => {
                    if let Err(err) = rpc_client.auto_sell(project.id).await {
                        match err {
                            MoonboisClientError::ServerError(err) => {
                                println!("\n{}\n  - {}", "Failed to auto sell tokens ⚠️".yellow(), err.dimmed());
                                Select::new()
                                    .items(&vec!["Back"])
                                    .default(0)
                                    .interact()
                                    .unwrap();
                            }
                            _ => panic!("{err}")
                        }
                    }
                }
                4 => {
                    if project.pumpfun.is_none() {
                        if project.pending_snipe {
                            rpc_client.cancel_snipe(project.id).await.unwrap();
                            project.pending_snipe = false;
                        } else {
                            let wallet_count = Input::new()
                                .with_prompt("Enter the amount of wallets to snipe")
                                .default(5)
                                .interact()
                                .unwrap();
                            project.pending_snipe = true;

                            if let Err(err) = rpc_client.create_snipe(project.id, wallet_count).await {
                                match err {
                                    MoonboisClientError::ServerError(err) => {
                                        project.pending_snipe = false;
                                        println!("\n{}\n  - {}", "Failed to create snipe ⚠️".yellow(), err.dimmed());
                                        Select::new()
                                            .items(&vec!["Back"])
                                            .default(0)
                                            .interact()
                                            .unwrap();
                                    }
                                    _ => panic!("{err}")
                                }
                            }
                        }
                    }
                }
                5 => {
                    if let Some(update) = match rpc_client.get_project(project.id).await {
                        Ok(project) => Some(project),
                        Err(err) => {
                            match err {
                                MoonboisClientError::ServerError(err) => {
                                    println!("\n{}\n  - {}", "Failed to update project ⚠️".yellow(), err.dimmed());
                                    Select::new()
                                        .items(&vec!["Back"])
                                        .default(0)
                                        .interact()
                                        .unwrap();

                                    None
                                }
                                _ => panic!("{err}")
                            }
                        }
                    } {
                        project = update;
                    }

                    continue
                },
                6 => {
                    let deleted = delete_project(&project, rpc_client).await;
                    if deleted { break }
                },
                7 => break,
                _ => ()
            }
    }
}

async fn load_project(rpc_client: &MoonboisClient) -> Option<ProjectDTO> {
    let projects = rpc_client.get_project_records().await.unwrap();
    let selection = projects.iter().map(|x| &x.name).collect::<Vec<&String>>();
    let length = selection.len();

    let index = Select::new()
        .with_prompt("Select Project")
        .default(0)
        .items(&vec![selection, vec![&"Back".to_string()]].concat())
        .interact()
        .unwrap();

    if index == length { return None; }

    let project = rpc_client.get_project(projects[index].id).await.unwrap();

    Some(project)
}

async fn create_project(rpc_client: &MoonboisClient) -> Option<ProjectDTO> {
    let deployer = loop {
        let deployer: String = Input::new()
            .with_prompt("Enter the deployer public key")
            .interact()
            .unwrap();

        if let Err(err) = Pubkey::from_str(&deployer) {
            println!("{err}");
            continue;
        }

        break Pubkey::from_str(&deployer).unwrap();
    };

    let name: String = loop {
        let value: String = Input::new()
            .with_prompt("Enter the project name")
            .allow_empty(false)
            .interact()
            .unwrap();

        if value.len() > 10 {
            println!("Name cannot use more than 10 characters");
            continue;
        }

        break value;
    };

    let project = match rpc_client.create_project(name, deployer).await {
        Ok(result) => result,
        Err(err) => {
            match err {
                MoonboisClientError::ServerError(err) => {
                    println!("\n{}\n  - {}", "Failed to create project ⚠️".yellow(), err.dimmed());
                    Select::new()
                        .items(&vec!["Back"])
                        .default(0)
                        .interact()
                        .unwrap();

                    return None
                },
                _ => panic!("{err}")
            }
        }
    };

    Some(project)
}

async fn delete_project(project: &ProjectDTO, rpc_client: &MoonboisClient) -> bool {
    let prompt = format!("{}", "Are you sure you want to delete this project? ⚠️".yellow());
    let delete = Confirm::new()
        .default(false)
        .with_prompt(prompt)
        .interact()
        .unwrap();
    
    if delete {
        rpc_client.delete_project(project.id).await.unwrap();
    }

    delete
}