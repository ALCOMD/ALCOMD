use crate::commands::prelude::*;
use crate::extensions::DISCORD_EXTENSION_ID;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub fn unity_discord_status(
    config: State<'_, GuiConfigState>,
    presence: State<'_, crate::discord_presence::DiscordPresenceState>,
) -> crate::discord_presence::UnityDiscordStatus {
    presence.status(config.get().is_extension_enabled(DISCORD_EXTENSION_ID))
}

#[tauri::command]
#[specta::specta]
pub async fn unity_discord_set_sharing_enabled(
    config: State<'_, GuiConfigState>,
    presence: State<'_, crate::discord_presence::DiscordPresenceState>,
    enabled: bool,
) -> Result<crate::discord_presence::UnityDiscordStatus, RustError> {
    let extension_enabled = {
        let mut config = config.load_mut().await?;
        config.unity_discord_sharing_enabled = enabled;
        let extension_enabled = config.is_extension_enabled(DISCORD_EXTENSION_ID);
        config.save().await?;
        extension_enabled
    };

    presence.set_sharing_enabled(enabled);
    Ok(presence.status(extension_enabled))
}
