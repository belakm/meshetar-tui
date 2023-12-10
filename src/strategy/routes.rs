use rocket::{response::status::Accepted, serde::json::Json, State};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{
    trading::meshetar::{Meshetar, MeshetarStatus},
    TaskControl,
};

use super::prediction_model;

#[post("/create_new_model")]
pub async fn create_new_model(
    meshetar: &State<Arc<Mutex<Meshetar>>>,
    task_control: &State<Arc<Mutex<TaskControl>>>,
) -> Accepted<Json<Meshetar>> {
    // Set state to running
    let meshetar_clone = Arc::clone(&meshetar.inner());
    meshetar_clone.lock().await.status = MeshetarStatus::CreatingNewModel;
    drop(meshetar_clone);
    // Set task control to running
    &task_control.lock().await.sender.send(true);
    let reciever = Arc::clone(&task_control.inner());

    let meshetar_clone2 = Arc::clone(&meshetar.inner());
    let meshetar_clone3 = Arc::clone(&meshetar.inner());
    // Start running
    tokio::spawn(async move {
        match prediction_model::create_model(reciever).await {
            Ok(_) => log::warn!("Created model successfully"),
            Err(e) => log::error!("Creating model failed with error {}", e),
        };
        let mut meshetar_clone = meshetar_clone2.lock().await;
        meshetar_clone.status = MeshetarStatus::Idle;
    });

    let summary = meshetar_clone3.lock().await.summerize_json();
    Accepted(Some(summary))
}
