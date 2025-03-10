 use dialoguer::theme::ColorfulTheme;
 use crate::dialogue::loader::Loader;
use std::sync::Arc;

use dialoguer::Confirm;
use dialoguer::FuzzySelect;
use dialoguer::Input;
use solana_sdk::pubkey::Pubkey;

use crate::AppData;
use crate::AppError;
use crate::Menu;

use super::bumps::BumpMenu;
use super::main::MainMenu;
use super::trade::Buy;
use super::trade::Sell;
use super::Handler;

pub enum ProjectMenuOptions {
    Buy,
    Sell,
    AutoBuy,
    AutoSell,
    Bumps,
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
            Self::Bumps => "Bumps".to_string(),
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
            4 => Self::Bumps,
            5 => Self::Delete,
            6 => Self::Back,
            _ => panic!("Received invalid project menu index")
        }
    }
}

pub struct ProjectMenu;
impl Handler for ProjectMenu {
    async fn handle(&self, _app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
        let mut items = vec![];

        items.push(ProjectMenuOptions::Buy);
        items.push(ProjectMenuOptions::Sell);
        items.push(ProjectMenuOptions::AutoBuy);
        items.push(ProjectMenuOptions::AutoSell);
        items.push(ProjectMenuOptions::Bumps);
        items.push(ProjectMenuOptions::Delete);
        items.push(ProjectMenuOptions::Back);

        let selection = match FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Project menu")
            .default(0)
            .items(items)
            .interact() {
                Ok(selection) => selection,
                Err(err) => {
                    return Err((Menu::Main(MainMenu), AppError::from(err)));
                }
            };

        match ProjectMenuOptions::from(selection) {
            ProjectMenuOptions::Buy => return Ok(Some(Menu::Buy(Buy::new(false)))),
            ProjectMenuOptions::Sell => return Ok(Some(Menu::Sell(Sell::new(false)))),
            ProjectMenuOptions::AutoBuy => return Ok(Some(Menu::Buy(Buy::new(true)))),
            ProjectMenuOptions::AutoSell => return Ok(Some(Menu::Sell(Sell::new(true)))),
            ProjectMenuOptions::Bumps => return Ok(Some(Menu::Bump(BumpMenu))),
            ProjectMenuOptions::Delete => return Ok(Some(Menu::DeleteProject(DeleteProject))),
            ProjectMenuOptions::Back => return Ok(Some(Menu::Main(MainMenu))),
        };
    }
}

pub struct SelectProject;
impl Handler for SelectProject {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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

pub struct CreateProject;
impl Handler for CreateProject {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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

pub struct DeleteProject;
impl Handler for DeleteProject {
    async fn handle(&self, app_data: &Arc<AppData>) -> Result<Option<Menu>, (Menu, AppError)> {
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