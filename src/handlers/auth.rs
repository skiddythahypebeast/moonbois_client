use dialoguer::theme::ColorfulTheme;
use crate::dialogue::loader::Loader;
use std::sync::Arc;

use dialoguer::Confirm;
use dialoguer::Input;
use moonbois_core::rpc::MoonboisClientError;
use moonbois_core::Credentials;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

use crate::AppData;
use crate::AppError;
use crate::Menu;

use super::main::MainMenu;
use super::Handler;

pub struct Login;
impl Handler for Login {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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

            return Err((Menu::Login(Login), AppError::from(err)));
        }

        let rpc_client = app_data.rpc_client.write().await;
        let get_user_reponse = Loader::new()
            .with_prompt("loading user")
            .interact(rpc_client.get_user())
            .await;
        drop(rpc_client);
    
        if let Err(err) = get_user_reponse {
            return Err((Menu::Login(Login), AppError::from(err)));
        } else if let Ok(user) = get_user_reponse {
            let mut user_write = app_data.user.write().await;
            user_write.0 = Some(user);
            return Ok(Some(Menu::Main(MainMenu)))
        }

        return Err((Menu::Login(Login), AppError::Unhandled("Unhandled error".to_string())));
    }
}

pub struct Signup {
    credentials: Credentials
}
impl Handler for Signup {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let user = self.credentials.signer.pubkey();
        let credentials = Credentials { signer: self.credentials.signer.insecure_clone() };
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