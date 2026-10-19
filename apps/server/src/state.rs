use crate::{store::Store, vault::Vault};
use anyhow::Result;
use isideload::{
    auth::apple_account::TwoFactorCallbackResponse,
    sideload::sideloader::Sideloader,
    util::callbacks::{ExtensionsBehaviorCallbackBox, MaxCertsCallbackBox},
};
use serde::Serialize;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::sync::{Mutex as AsyncMutex, oneshot};

pub type Engine = Sideloader<MaxCertsCallbackBox, ExtensionsBehaviorCallbackBox>;

#[derive(Clone, Serialize)]
pub struct TeamView {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Serialize)]
pub struct NumberView {
    pub id: u32,
    pub label: String,
}
#[derive(Clone, Serialize)]
pub struct ChallengeView {
    pub id: String,
    pub sms: bool,
    pub numbers: Vec<NumberView>,
    pub unknown: bool,
}
#[derive(Clone, Serialize)]
pub struct AccountView {
    pub status: String,
    pub email: Option<String>,
    pub message: Option<String>,
    pub teams: Vec<TeamView>,
    pub selected_team: Option<String>,
    pub challenge: Option<ChallengeView>,
}
impl Default for AccountView {
    fn default() -> Self {
        Self {
            status: "logged_out".into(),
            email: None,
            message: None,
            teams: vec![],
            selected_team: None,
            challenge: None,
        }
    }
}

pub struct PendingChallenge {
    pub id: String,
    pub sender: oneshot::Sender<TwoFactorCallbackResponse>,
}

pub struct AppState {
    pub store: Mutex<Store>,
    pub vault: Vault,
    pub account: Mutex<AccountView>,
    pub challenge: Mutex<Option<PendingChallenge>>,
    pub engine: AsyncMutex<Option<Engine>>,
    pub operation: Arc<AsyncMutex<()>>,
    pub shutdown: tokio::sync::Notify,
    pub network: Mutex<crate::companion::NetworkView>,
    pub clients: Mutex<Vec<crate::companion::Client>>,
    pub token: String,
    pub authority: String,
}

impl AppState {
    pub fn new(root: &Path, authority: String, token: String) -> Result<Arc<Self>> {
        use isideload::util::storage::SideloadingStorage;
        let store = Store::open(root)?;
        let vault = Vault::open(&root.join("vault"))?;
        let clients = match vault
            .retrieve("companion-clients")
            .map_err(|_| anyhow::anyhow!("Nie można odczytać zaufanych klientów."))?
        {
            Some(data) => serde_json::from_str(&data)?,
            None => vec![],
        };
        Ok(Arc::new(Self {
            store: Mutex::new(store),
            vault,
            account: Mutex::new(AccountView::default()),
            challenge: Mutex::new(None),
            engine: AsyncMutex::new(None),
            operation: Arc::new(AsyncMutex::new(())),
            shutdown: tokio::sync::Notify::new(),
            network: Mutex::new(Default::default()),
            clients: Mutex::new(clients),
            token,
            authority,
        }))
    }
}
