use anyhow::Result;
use axum::{extract::State, response::IntoResponse, Form};
use axum_extra::response::Html;
use serde::Deserialize;

use crate::{
    errors::SupercellError,
    storage::{denylist_remove, denylist_upsert, feed_content_purge_aturi},
};

use super::context::WebContext;

#[derive(Deserialize, Default)]
pub struct AdminForm {
    pub action: Option<String>,
    pub did: Option<String>,
    pub reason: Option<String>,
    pub aturi: Option<String>,
    pub feed: Option<String>,
}

pub async fn handle_admin(
    State(web_context): State<WebContext>,
    Form(form): Form<AdminForm>,
) -> Result<impl IntoResponse, SupercellError> {
    if let Some(action) = form.action {
        match action.as_str() {
            "purge" => {
                if let Some(aturi) = form.aturi {
                    let feed = form.feed.filter(|s| !s.is_empty());
                    tracing::debug!("purging at-uri: {:?} with feed: {:?}", aturi, feed);
                    feed_content_purge_aturi(&web_context.pool, &aturi, &feed).await?;
                }
            }
            "deny" => {
                if let Some(did) = form.did {
                    let reason = form.reason.unwrap_or("n/a".to_string());
                    denylist_upsert(&web_context.pool, &did, &reason).await?;
                }
            }
            "allow" => {
                if let Some(did) = form.did {
                    denylist_remove(&web_context.pool, &did).await?;
                }
            }
            _ => {}
        }
    }

    Ok(Html(
        r#"
        <!doctype html>
        <html>
            <head><title>Supercell Admin</title></head>
            <body>
                <p>Purge AT-URI</p>
                <form action="/admin" method="post">
                    <input type="hidden" name="action" value="purge">
                    <label for="purge_aturi">AT-URI: <input type="text" id="purge_aturi" name="aturi" required="required"></label>
                    <label for="purge_feed">Feed (optional): <input type="text" id="purge_feed" name="feed"></label>
                    <input type="submit" name="submit" value="Submit">
                </form>
                <hr/>
                <p>Denylist Add</p>
                <form action="/admin" method="post">
                    <input type="hidden" name="action" value="deny">
                    <label for="deny_did">DID: <input type="text" id="deny_did" name="did" required="required"></label>
                    <label for="deny_reason">Reason (optional): <input type="text" id="deny_reason" name="reason"></label>
                    <input type="submit" name="submit" value="Submit">
                </form>
                <hr/>
                <p>Denylist Remove</p>
                <form action="/admin" method="post">
                    <input type="hidden" name="action" value="allow">
                    <label for="allow_did">DID: <input type="text" id="allow_did" name="did" required="required"></label>
                    <input type="submit" name="submit" value="Submit">
                </form>
                <hr/>
            </body>
        </html>
        "#,
    ))
}
