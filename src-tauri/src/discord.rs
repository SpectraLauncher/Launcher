use discord_rich_presence::activity::Activity;
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use crate::AppState;

const CLIENT_ID: &str = env!("DISCORD_CLIENT_ID");

pub fn update_presence(state: &AppState) {
    let snapshot: Vec<(String, String)> = match state.discord_playing.lock() {
        Ok(map) => map.values().cloned().collect(),
        Err(_) => return,
    };

    let Ok(mut guard) = state.discord.lock() else { return };

    if snapshot.is_empty() {
        if let Some(client) = guard.as_mut() {
            let _ = client.clear_activity();
        }
        return;
    }

    if guard.is_none() {
        if let Ok(mut client) = DiscordIpcClient::new(CLIENT_ID) {
            if client.connect().is_ok() {
                *guard = Some(client);
            }
        }
    }

    if let Some(client) = guard.as_mut() {
        let (details, line) = if snapshot.len() == 1 {
            let (name, mc) = &snapshot[0];
            (format!("Playing {name}"), format!("Minecraft {mc}"))
        } else {
            let (name, mc) = &snapshot[0];
            (
                format!("Playing {} instances", snapshot.len()),
                format!("{name} · Minecraft {mc}"),
            )
        };
        let _ = client.set_activity(Activity::new().details(&details).state(&line));
    }
}
