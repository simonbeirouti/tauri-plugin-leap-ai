use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::LeapAi;
#[cfg(mobile)]
use mobile::LeapAi;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the leap-ai APIs.
pub trait LeapAiExt<R: Runtime> {
    fn leap_ai(&self) -> &LeapAi<R>;
}

impl<R: Runtime, T: Manager<R>> crate::LeapAiExt<R> for T {
    fn leap_ai(&self) -> &LeapAi<R> {
        self.state::<LeapAi<R>>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("leap-ai")
        .invoke_handler(tauri::generate_handler![
            commands::download_model,
            commands::load_model,
            commands::load_cached_model,
            commands::list_cached_models,
            commands::remove_cached_model,
            commands::unload_model,
            commands::create_conversation,
            commands::create_conversation_from_history,
            commands::generate,
            commands::stop_generation,
            commands::export_conversation,
            commands::runtime_info
        ])
        .setup(|app, api| {
            #[cfg(mobile)]
            let leap_ai = mobile::init(app, api)?;
            #[cfg(desktop)]
            let leap_ai = desktop::init(app, api)?;
            app.manage(leap_ai);
            Ok(())
        })
        .build()
}
