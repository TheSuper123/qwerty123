use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn bad(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
        }
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "operation_conflict",
            message: message.into(),
        }
    }
    pub fn internal() -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, code: "storage_error", message: "Nie można zapisać stanu programu. Sprawdź wolne miejsce i uprawnienia katalogu danych.".into() }
    }
    pub fn device() -> Self {
        Self { status: StatusCode::SERVICE_UNAVAILABLE, code: "device_unavailable", message: "Nie można połączyć się z iPhonem. Sprawdź Apple Mobile Device Service, kabel, odblokowanie telefonu i zgodę Zaufaj.".into() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"code":self.code,"message":self.message})),
        )
            .into_response()
    }
}

pub fn account_message(error: &str) -> &'static str {
    let error = error.to_lowercase();
    if error.contains("anisette")
        || error.contains("client_info")
        || error.contains("provisioning socket")
    {
        "Nie można uzyskać danych anisette. Uruchom własną usługę v3 lub sprawdź jawnie wybrany adres i jego certyfikat."
    } else if error.contains("certificate")
        && (error.contains("maximum") || error.contains("limit") || error.contains("max"))
    {
        "Osiągnięto limit certyfikatów Apple. Most nie odwołuje cudzych certyfikatów automatycznie. Zarządzaj nimi na koncie deweloperskim."
    } else if error.contains("two.factor")
        || error.contains("2fa")
        || error.contains("verification")
    {
        "Weryfikacja Apple nie powiodła się. Zaloguj się ponownie i sprawdź kod 2FA."
    } else if error.contains("entitlement")
        || error.contains("provisioning")
        || error.contains("app id")
    {
        "Apple nie pozwoliło przygotować profilu lub uprawnień tej aplikacji. Sprawdź limity konta i zgodność IPA."
    } else if error.contains("password")
        || error.contains("credential")
        || error.contains("authentication")
    {
        "Apple odrzuciło logowanie. Sprawdź dane konta oraz wymagane potwierdzenia na zaufanym urządzeniu."
    } else {
        "Operacja Apple nie powiodła się. Sprawdź logowanie, usługę anisette, połączenie internetowe i limity konta. Szczegóły konta nie są zapisywane w logach."
    }
}
