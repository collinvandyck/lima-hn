use rusqlite::Connection;
use std::path::PathBuf;
use tokio::sync::mpsc;

use super::migrations::run_migrations;
use super::queries;
use super::{StorageCommand, StorageError};
use anyhow::Result;

pub fn worker(
    db_path: PathBuf,
    cmd_rx: mpsc::Receiver<StorageCommand>,
) -> Result<(), StorageError> {
    let parent = db_path.parent().ok_or(StorageError::NoDbPathParent)?;
    if !parent.exists() {
        std::fs::create_dir_all(parent).map_err(StorageError::IO)?;
    }
    let conn = Connection::open(&db_path)?;
    run_migrations(&conn)?;
    std::thread::spawn(move || {
        run_worker(conn, cmd_rx);
    });
    Ok(())
}

#[cfg(test)]
pub fn worker_in_memory(cmd_rx: mpsc::Receiver<StorageCommand>) {
    let conn = Connection::open_in_memory().expect("Failed to open in-memory database");
    run_migrations(&conn).expect("Failed to run migrations");
    run_worker(conn, cmd_rx);
}

fn run_worker(conn: Connection, mut cmd_rx: mpsc::Receiver<StorageCommand>) {
    while let Some(cmd) = cmd_rx.blocking_recv() {
        match cmd {
            StorageCommand::SaveStory { story, reply } => {
                let result = queries::save_story(&conn, &story);
                let _ = reply.send(result);
            }
            StorageCommand::GetStory { id, reply } => {
                let result = queries::get_story(&conn, id);
                let _ = reply.send(result);
            }
            StorageCommand::SaveComments {
                story_id,
                comments,
                reply,
            } => {
                let result = queries::save_comments(&conn, story_id, &comments);
                let _ = reply.send(result);
            }
            StorageCommand::GetComments { story_id, reply } => {
                let result = queries::get_comments(&conn, story_id);
                let _ = reply.send(result);
            }
            StorageCommand::SaveFeed { feed, ids, reply } => {
                let result = queries::save_feed(&conn, feed, &ids);
                let _ = reply.send(result);
            }
            StorageCommand::GetFeed { feed, reply } => {
                let result = queries::get_feed(&conn, feed);
                let _ = reply.send(result);
            }
        }
    }
}
