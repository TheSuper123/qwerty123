use crate::models::{Device, InstalledApp};
use anyhow::{Context, Result, ensure};
use idevice::{
    IdeviceService,
    installation_proxy::InstallationProxyClient,
    lockdown::LockdownClient,
    provider::{IdeviceProvider, UsbmuxdProvider},
    usbmuxd::{Connection, UsbmuxdAddr, UsbmuxdConnection},
};
use serde::Serialize;
use std::time::Duration;

#[derive(Serialize)]
pub struct DeviceReport {
    pub available: bool,
    pub devices: Vec<Device>,
    pub message: Option<String>,
}

pub async fn provider(id: &str) -> Result<UsbmuxdProvider> {
    let mut connection = UsbmuxdConnection::default().await?;
    let mut devices = connection.get_devices().await?;
    devices.sort_by_key(|d| !matches!(d.connection_type, Connection::Usb));
    let device = devices
        .into_iter()
        .find(|d| d.udid == id)
        .context("Nie znaleziono urządzenia.")?;
    Ok(device.to_provider(UsbmuxdAddr::from_env_var()?, "MostServer"))
}

pub async fn discover() -> DeviceReport {
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let mut connection = UsbmuxdConnection::default().await?;
        connection.get_devices().await
    })
    .await;
    let mut found = match result {
        Ok(Ok(devices)) => devices,
        _ => return DeviceReport { available: false, devices: vec![], message: Some("Brak połączenia z Apple Mobile Device Service. Zainstaluj oficjalne sterowniki Apple/iTunes, uruchom usługę i podłącz odblokowany iPhone.".into()) },
    };
    found.sort_by_key(|d| !matches!(d.connection_type, Connection::Usb));
    let mut seen = std::collections::HashSet::new();
    let mut devices = vec![];
    for item in found
        .into_iter()
        .filter(|d| seen.insert(d.udid.clone()))
        .take(16)
    {
        let transport = if matches!(item.connection_type, Connection::Usb) {
            "USB"
        } else {
            "Wi-Fi"
        };
        let mut view = Device {
            id: item.udid.clone(),
            name: "iPhone / iPad".into(),
            ios: None,
            connection: transport.into(),
            trusted: false,
        };
        if let Ok(addr) = UsbmuxdAddr::from_env_var() {
            let p = item.to_provider(addr, "MostServer");
            if let Ok(Ok((name, ios))) = tokio::time::timeout(Duration::from_secs(2), async {
                let mut lockdown = LockdownClient::connect(&p).await?;
                lockdown.start_session(&p.get_pairing_file().await?).await?;
                let name = lockdown.get_value(Some("DeviceName"), None).await?;
                let ios = lockdown.get_value(Some("ProductVersion"), None).await?;
                Ok::<_, idevice::IdeviceError>((
                    name.as_string().unwrap_or("iPhone").to_owned(),
                    ios.as_string().map(str::to_owned),
                ))
            })
            .await
            {
                view.name = name;
                view.ios = ios;
                view.trusted = true;
            }
        }
        devices.push(view);
    }
    DeviceReport {
        available: true,
        devices,
        message: None,
    }
}

pub async fn pair(id: &str) -> Result<()> {
    let p = provider(id).await?;
    let mut mux = UsbmuxdConnection::default().await?;
    let mut lockdown = LockdownClient::connect(&p).await?;
    let buid = mux.get_buid().await?;
    let record = lockdown
        .pair(uuid::Uuid::new_v4().to_string(), buid, Some("MostServer"))
        .await?;
    mux.save_pair_record(id, record.serialize()?).await?;
    Ok(())
}

pub async fn apps(id: &str) -> Result<Vec<InstalledApp>> {
    let p = provider(id).await?;
    let mut client = InstallationProxyClient::connect(&p).await?;
    let apps = client.get_apps(Some("User"), None).await?;
    let mut result = vec![];
    for (bundle_id, value) in apps {
        let Some(info) = value.as_dictionary() else {
            continue;
        };
        let field = |key| {
            info.get(key)
                .and_then(plist::Value::as_string)
                .unwrap_or("")
                .to_owned()
        };
        result.push(InstalledApp {
            bundle_id,
            name: info
                .get("CFBundleDisplayName")
                .or_else(|| info.get("CFBundleName"))
                .and_then(plist::Value::as_string)
                .unwrap_or("Aplikacja")
                .to_owned(),
            version: field("CFBundleShortVersionString"),
            build: field("CFBundleVersion"),
        });
    }
    result.sort_by_key(|a| a.name.to_lowercase());
    Ok(result)
}

pub async fn verify(id: &str, bundle: &str, build: &str) -> Result<()> {
    let p = provider(id).await?;
    let mut client = InstallationProxyClient::connect(&p).await?;
    let result = client.get_apps(None, Some(vec![bundle.to_owned()])).await?;
    let info = result
        .get(bundle)
        .and_then(plist::Value::as_dictionary)
        .context("Nie potwierdzono aplikacji na urządzeniu.")?;
    ensure!(
        info.get("CFBundleVersion")
            .and_then(plist::Value::as_string)
            == Some(build),
        "Nie potwierdzono właściwego buildu na urządzeniu."
    );
    Ok(())
}
