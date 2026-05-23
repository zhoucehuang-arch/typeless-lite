use anyhow::Result;

const SERVICE_NAME: &str = "com.typeless.lite";
const LLM_API_KEY_ACCOUNT: &str = "llm.api_key";

pub struct CredentialsVault;

impl CredentialsVault {
    pub fn get_llm_api_key() -> Option<String> {
        keyring::Entry::new(SERVICE_NAME, LLM_API_KEY_ACCOUNT)
            .ok()
            .and_then(|entry| entry.get_password().ok())
            .filter(|value| !value.trim().is_empty())
    }

    pub fn set_llm_api_key(value: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, LLM_API_KEY_ACCOUNT)?;
        if value.trim().is_empty() {
            let _ = entry.delete_credential();
        } else {
            entry.set_password(value)?;
        }
        Ok(())
    }

    pub fn llm_configured() -> bool {
        Self::get_llm_api_key().is_some()
    }
}
