use crate::{
    error::account_message,
    state::{AccountView, AppState, ChallengeView, NumberView, PendingChallenge, TeamView},
};
use anyhow::{Result, ensure};
use isideload::{
    anisette::remote_v3::RemoteV3AnisetteProvider,
    auth::apple_account::{AppleAccount, TwoFactorCallbackParams, TwoFactorCallbackResponse},
    dev::{
        developer_session::DeveloperSession,
        teams::{DeveloperTeam, TeamsApi},
    },
    sideload::{SideloaderBuilder, TeamSelection, builder::MaxCertsBehavior},
    util::callbacks::{ExtensionsBehaviorCallbackBox, MaxCertsCallbackBox},
};
use serde::Deserialize;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use zeroize::Zeroizing;

// isideload accepts a function pointer for team selection. MostServer serves exactly one
// interactive account; the operation mutex serializes login, team changes and signing.
static SELECTED_TEAM: Mutex<Option<String>> = Mutex::new(None);
#[expect(
    clippy::ptr_arg,
    reason = "isideload's TeamSelection requires this exact function-pointer signature"
)]
fn select_team(teams: &Vec<DeveloperTeam>) -> Option<String> {
    let selected = SELECTED_TEAM.lock().ok()?.clone()?;
    teams
        .iter()
        .find(|t| t.team_id == selected)
        .map(|t| t.team_id.clone())
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub anisette_url: String,
    #[serde(default)]
    pub allow_external: bool,
}

pub fn validate_anisette(value: &str, allow_external: bool) -> Result<String> {
    let url = url::Url::parse(value)?;
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Adres anisette nie może zawierać danych logowania, parametrów ani fragmentu."
    );
    let loopback = matches!(url.host_str(), Some("127.0.0.1") | Some("[::1]"));
    ensure!(
        (url.scheme() == "http" && loopback) || url.scheme() == "https",
        "Użyj lokalnego http://127.0.0.1:6969 albo adresu HTTPS z poprawnym certyfikatem."
    );
    ensure!(
        loopback || allow_external,
        "Zewnętrzna usługa anisette wymaga świadomej zgody w formularzu."
    );
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

pub async fn login(state: Arc<AppState>, request: LoginRequest) -> Result<()> {
    let password = Zeroizing::new(request.password);
    let provider = RemoteV3AnisetteProvider::new(
        &request.anisette_url,
        Box::new(state.vault.clone()),
        "0".to_owned(),
    )
    .map_err(|_| anyhow::anyhow!("anisette client unavailable"))?;
    let callback_state = state.clone();
    let callback = move |params: TwoFactorCallbackParams| {
        let state = callback_state.clone();
        async move {
            let id = uuid::Uuid::new_v4().to_string();
            let (sender, receiver) = tokio::sync::oneshot::channel();
            {
                *state
                    .challenge
                    .lock()
                    .map_err(|_| rootcause::report!("Blokada stanu 2FA."))? =
                    Some(PendingChallenge {
                        id: id.clone(),
                        sender,
                    });
                let mut view = state
                    .account
                    .lock()
                    .map_err(|_| rootcause::report!("Blokada stanu konta."))?;
                view.status = "two_factor".into();
                view.challenge = Some(ChallengeView {
                    id,
                    sms: params.sms,
                    unknown: params.unknown,
                    numbers: params
                        .numbers
                        .into_iter()
                        .map(|n| NumberView {
                            id: n.id,
                            label: format!("Numer kończący się na {}", n.last_two_digits),
                        })
                        .collect(),
                });
            }
            let response = tokio::time::timeout(Duration::from_secs(300), receiver)
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(TwoFactorCallbackResponse::Abort);
            *state
                .challenge
                .lock()
                .map_err(|_| rootcause::report!("Blokada stanu 2FA."))? = None;
            if let Ok(mut view) = state.account.lock() {
                view.status = "logging_in".into();
                view.challenge = None;
            }
            Ok(response)
        }
    };
    let mut account = AppleAccount::builder(&request.email)
        .anisette_provider(provider)
        .err_429_retries(Some(1))
        .login(&password, callback)
        .await
        .map_err(|e| anyhow::anyhow!("{}", account_message(&e.to_string())))?;
    let mut session = DeveloperSession::from_account(&mut account)
        .await
        .map_err(|e| anyhow::anyhow!("{}", account_message(&e.to_string())))?;
    let teams = session
        .list_teams()
        .await
        .map_err(|e| anyhow::anyhow!("{}", account_message(&e.to_string())))?;
    ensure!(
        !teams.is_empty(),
        "Na koncie nie ma dostępnego zespołu deweloperskiego."
    );
    let selected = if teams.len() == 1 {
        Some(teams[0].team_id.clone())
    } else {
        None
    };
    *SELECTED_TEAM
        .lock()
        .map_err(|_| anyhow::anyhow!("Blokada zespołu."))? = selected.clone();
    let engine = SideloaderBuilder::<MaxCertsCallbackBox, ExtensionsBehaviorCallbackBox>::new(
        session,
        request.email.clone(),
    )
    .team_selection(TeamSelection::PromptOnce(select_team))
    .max_certs_behavior(MaxCertsBehavior::Error)
    .machine_name("MostServer".into())
    .storage(Box::new(state.vault.clone()))
    .build();
    *state.engine.lock().await = Some(engine);
    *state
        .account
        .lock()
        .map_err(|_| anyhow::anyhow!("Blokada konta."))? = AccountView {
        status: "logged_in".into(),
        email: Some(request.email),
        message: None,
        teams: teams
            .into_iter()
            .map(|t| TeamView {
                name: t.name.unwrap_or_else(|| t.team_id.clone()),
                id: t.team_id,
            })
            .collect(),
        selected_team: selected,
        challenge: None,
    };
    Ok(())
}

pub fn choose_team(state: &AppState, id: &str) -> Result<()> {
    let mut view = state
        .account
        .lock()
        .map_err(|_| anyhow::anyhow!("Blokada konta."))?;
    ensure!(
        view.status == "logged_in" && view.teams.iter().any(|t| t.id == id),
        "Zespół nie jest dostępny na tym koncie."
    );
    ensure!(
        view.selected_team.is_none() || view.selected_team.as_deref() == Some(id),
        "Aby zmienić aktywny zespół, wyloguj się i zaloguj ponownie."
    );
    *SELECTED_TEAM
        .lock()
        .map_err(|_| anyhow::anyhow!("Blokada zespołu."))? = Some(id.to_owned());
    view.selected_team = Some(id.to_owned());
    Ok(())
}
