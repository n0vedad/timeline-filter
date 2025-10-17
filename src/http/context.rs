use axum::extract::FromRef;
use std::{
    ops::Deref,
    sync::Arc,
};

use crate::storage::StoragePool;

pub struct InnerWebContext {
    pub(crate) pool: StoragePool,
    pub(crate) external_base: String,
}

#[derive(Clone, FromRef)]
pub struct WebContext(pub(crate) Arc<InnerWebContext>);

impl Deref for WebContext {
    type Target = InnerWebContext;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl WebContext {
    pub fn new(
        pool: StoragePool,
        external_base: &str,
    ) -> Self {
        Self(Arc::new(InnerWebContext {
            pool,
            external_base: external_base.to_string(),
        }))
    }
}
