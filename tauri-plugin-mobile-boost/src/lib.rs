use tauri::{
    Manager, Runtime,
    plugin::{Builder, TauriPlugin},
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
use desktop::MobileBoost;
#[cfg(mobile)]
use mobile::MobileBoost;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the mobile-boost APIs.
pub trait MobileBoostExt<R: Runtime> {
    fn mobile_boost(&self) -> &MobileBoost<R>;
}

impl<R: Runtime, T: Manager<R>> crate::MobileBoostExt<R> for T {
    fn mobile_boost(&self) -> &MobileBoost<R> {
        self.state::<MobileBoost<R>>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("mobile-boost")
        .invoke_handler(tauri::generate_handler![commands::ping])
        .setup(|app, api| {
            #[cfg(mobile)]
            let mobile_boost = mobile::init(app, api)?;
            #[cfg(desktop)]
            let mobile_boost = desktop::init(app, api)?;
            app.manage(mobile_boost);
            Ok(())
        })
        .build()
}
