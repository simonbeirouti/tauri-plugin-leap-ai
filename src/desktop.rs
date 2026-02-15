use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
  app: &AppHandle<R>,
  _api: PluginApi<R, C>,
) -> crate::Result<LeapAi<R>> {
  Ok(LeapAi(app.clone()))
}

/// Access to the leap-ai APIs.
pub struct LeapAi<R: Runtime>(AppHandle<R>);

impl<R: Runtime> LeapAi<R> {
  pub fn ping(&self, payload: PingRequest) -> crate::Result<PingResponse> {
    Ok(PingResponse {
      value: payload.value,
    })
  }
}
