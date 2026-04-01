use std::fs;
use std::path::Path;

use solana_sdk::signer::Signer;
use solana_sdk::signer::keypair::Keypair;

use crate::error::AwError;

pub fn load_keypair(path: &Path) -> Result<Keypair, AwError> {
    let data = fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AwError::Wallet(format!(
                "keypair not found at {}\n  → run `aw wallet new` to generate one",
                path.display()
            ))
        } else {
            AwError::Wallet(format!("failed to read keypair at {}: {e}", path.display()))
        }
    })?;

    let bytes: Vec<u8> = serde_json::from_str(&data).map_err(|e| {
        AwError::Wallet(format!("invalid keypair format at {}: {e}", path.display()))
    })?;

    Keypair::try_from(bytes.as_slice())
        .map_err(|e| AwError::Wallet(format!("invalid keypair bytes at {}: {e}", path.display())))
}

pub fn new_keypair(output_path: Option<&std::path::Path>) -> Result<(), AwError> {
    let keypair_path = match output_path {
        Some(p) => p.to_path_buf(),
        None => default_keypair_dir()?.join("id.json"),
    };

    if keypair_path.exists() {
        return Err(AwError::Wallet(format!(
            "keypair already exists at {}\n  → use --keypair to specify a different path",
            keypair_path.display()
        )));
    }

    if let Some(parent) = keypair_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AwError::Wallet(format!("failed to create {}: {e}", parent.display())))?;
    }

    let keypair = Keypair::new();
    let bytes: Vec<u8> = keypair.to_bytes().to_vec();
    let json = serde_json::to_string(&bytes)
        .map_err(|e| AwError::Wallet(format!("failed to serialize keypair: {e}")))?;

    fs::write(&keypair_path, &json)
        .map_err(|e| AwError::Wallet(format!("failed to write keypair: {e}")))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&keypair_path, fs::Permissions::from_mode(0o600))
            .map_err(|e| AwError::Wallet(format!("failed to set keypair permissions: {e}")))?;
    }

    eprintln!("{}", keypair.pubkey());
    Ok(())
}

pub fn import_keypair(source: &Path) -> Result<(), AwError> {
    // Validate source is a valid keypair
    let _ = load_keypair(source)?;

    let dest = default_keypair_dir()?.join("id.json");
    if dest.exists() {
        return Err(AwError::Wallet(format!(
            "keypair already exists at {}\n  → use --keypair to specify a different path",
            dest.display()
        )));
    }

    let parent = dest.parent().unwrap();
    fs::create_dir_all(parent)
        .map_err(|e| AwError::Wallet(format!("failed to create {}: {e}", parent.display())))?;

    fs::copy(source, &dest).map_err(|e| AwError::Wallet(format!("failed to copy keypair: {e}")))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o600))
            .map_err(|e| AwError::Wallet(format!("failed to set keypair permissions: {e}")))?;
    }

    let keypair = load_keypair(&dest)?;
    eprintln!("{}", keypair.pubkey());
    Ok(())
}

pub fn show_pubkey(path: &Path) -> Result<(), AwError> {
    let keypair = load_keypair(path)?;
    println!("{}", keypair.pubkey());
    Ok(())
}

fn default_keypair_dir() -> Result<std::path::PathBuf, AwError> {
    let home = std::env::var("HOME")
        .map_err(|_| AwError::Config("HOME environment variable not set".into()))?;
    Ok(std::path::PathBuf::from(home).join(".config/solana"))
}
