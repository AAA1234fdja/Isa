use super::Result;
use reqwest::blocking::Client;
use serde_json::Value;
use std::time::{Duration, Instant};

const SERVICE: &str = "dev.isa.connectors";
const TOKEN_KEY: &str = "github.access_token";
const ENABLED_KEY: &str = "github.enabled";
const VERIFY_URL: &str = "https://github.com/login/device";

fn entry(name: &str) -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(SERVICE, name)?)
}

fn client() -> Result<Client> {
    Ok(Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20)).user_agent("ISA-CLI/0.1").build()?)
}

fn client_id() -> Result<String> {
    let id = std::env::var("ISA_GITHUB_CLIENT_ID").map_err(|_| "ISA_GITHUB_CLIENT_ID must be set to a registered GitHub OAuth app client ID with device flow enabled")?;
    if id.is_empty() || id.len() > 200 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        return Err("invalid GitHub client ID".into());
    }
    Ok(id)
}

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value.get(key).and_then(Value::as_str).filter(|value| !value.is_empty()).ok_or_else(|| format!("GitHub returned an invalid {key}").into())
}

fn verify_user(client: &Client, token: &str) -> Result<String> {
    let response: Value = client.get("https://api.github.com/user")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .bearer_auth(token).send()?.error_for_status()?.json()?;
    let login = string(&response, "login")?;
    if login.len() > 100 || !login.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') { return Err("GitHub returned an invalid account name".into()); }
    Ok(login.to_string())
}

pub fn connect_github() -> Result<()> {
    let id = client_id()?;
    let token_entry = entry(TOKEN_KEY)?;
    let enabled_entry = entry(ENABLED_KEY)?;
    match token_entry.get_password() {
        Ok(_) => return Err("GitHub is already connected; disconnect before connecting a different account".into()),
        Err(keyring::Error::NoEntry) => (),
        Err(error) => return Err(error.into()),
    }
    let client = client()?;
    let response: Value = client.post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[("client_id", id.as_str()), ("scope", "read:user")])
        .send()?.error_for_status()?.json()?;
    let device_code = string(&response, "device_code")?;
    let user_code = string(&response, "user_code")?;
    let verification_uri = string(&response, "verification_uri")?;
    if verification_uri != VERIFY_URL || user_code.len() > 40 { return Err("unexpected GitHub device verification response".into()); }
    let expires = response.get("expires_in").and_then(Value::as_u64).filter(|seconds| *seconds > 0 && *seconds <= 1800).ok_or("invalid device code expiry")?;
    let mut interval = response.get("interval").and_then(Value::as_u64).unwrap_or(5).max(5);
    if interval > 60 { return Err("invalid device polling interval".into()); }
    println!("Open {VERIFY_URL} and enter the displayed code: {user_code}");
    println!("Waiting for GitHub authorization; this requests read-only profile access.");
    let deadline = Instant::now() + Duration::from_secs(expires);
    loop {
        let now = Instant::now();
        if now >= deadline || deadline.duration_since(now) < Duration::from_secs(interval) { return Err("GitHub device authorization expired".into()); }
        std::thread::sleep(Duration::from_secs(interval));
        let response: Value = client.post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[("client_id", id.as_str()), ("device_code", device_code), ("grant_type", "urn:ietf:params:oauth:grant-type:device_code")])
            .send()?.error_for_status()?.json()?;
        if let Some(token) = response.get("access_token").and_then(Value::as_str) {
            if token.is_empty() || token.len() > 4096 { return Err("GitHub returned an invalid access token".into()); }
            let username = verify_user(&client, token)?;
            token_entry.set_password(token)?;
            if let Err(error) = enabled_entry.set_password("true") {
                let _ = token_entry.delete_credential();
                return Err(error.into());
            }
            println!("Connected GitHub account {username}. Credentials are stored in the operating system credential store.");
            return Ok(());
        }
        match response.get("error").and_then(Value::as_str).unwrap_or("unknown_error") {
            "authorization_pending" => (),
            "slow_down" => interval = interval.saturating_add(5),
            "access_denied" => return Err("GitHub authorization was denied".into()),
            "expired_token" => return Err("GitHub device authorization expired".into()),
            _ => return Err("GitHub device authorization failed".into()),
        }
    }
}

pub fn github_status() -> Result<()> {
    let token = entry(TOKEN_KEY)?.get_password();
    match token {
        Ok(_) => {
            let enabled = entry(ENABLED_KEY)?.get_password().unwrap_or_default() == "true";
            println!("GitHub: connected locally, {}", if enabled { "enabled" } else { "disabled" });
            println!("Authorization may have expired or been revoked; use whoami to verify it.");
            Ok(())
        }
        Err(keyring::Error::NoEntry) => { println!("GitHub: not connected"); Ok(()) }
        Err(error) => Err(error.into()),
    }
}

pub fn set_github_enabled(enabled: bool) -> Result<()> {
    let token = entry(TOKEN_KEY)?.get_password()?;
    if enabled { verify_user(&client()?, &token)?; }
    entry(ENABLED_KEY)?.set_password(if enabled { "true" } else { "false" })?;
    println!("GitHub connector {}.", if enabled { "enabled" } else { "disabled" });
    Ok(())
}

pub fn github_whoami() -> Result<()> {
    if entry(ENABLED_KEY)?.get_password().unwrap_or_default() != "true" { return Err("GitHub connector is disabled or not connected".into()); }
    let token = entry(TOKEN_KEY)?.get_password()?;
    println!("{}", verify_user(&client()?, &token)?);
    Ok(())
}

pub fn disconnect_github() -> Result<()> {
    for name in [ENABLED_KEY, TOKEN_KEY] {
        match entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => (),
            Err(error) => return Err(error.into()),
        }
    }
    println!("Local GitHub credentials removed. Revoke ISA's access on GitHub to invalidate the authorization.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn response_fields_are_required() {
        assert!(string(&serde_json::json!({"code": ""}), "code").is_err());
        assert!(string(&serde_json::json!({}), "code").is_err());
        assert_eq!(string(&serde_json::json!({"code": "test"}), "code").unwrap(), "test");
    }
}
