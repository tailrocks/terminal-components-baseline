#![allow(dead_code)]

use jackin_console::backend::{
    FieldDescriptor, OnePassword, OpAccount, OpError, OpField, OpItem, OpSession, OpVault,
    SecretClass,
};
use jackin_console::domain::onepassword::OpReference;

pub const NOT_USED: &str = "the product-shape test does not reach this";

#[derive(Debug, Clone)]
pub struct SignedInOnly;

impl OnePassword for SignedInOnly {
    fn session(&self) -> OpSession {
        OpSession::SignedIn
    }

    fn set_session(&mut self, _: OpSession) {}

    fn accounts(&self) -> Vec<OpAccount> {
        Vec::new()
    }

    fn latency_ms(&self) -> i64 {
        0
    }

    fn list_accounts(&self) -> Result<Vec<&OpAccount>, OpError> {
        Ok(Vec::new())
    }

    fn list_vaults(&self, _: &str) -> Result<Vec<&OpVault>, OpError> {
        unreachable!("{NOT_USED}")
    }

    fn list_items(&self, _: &str, _: &str) -> Result<Vec<&OpItem>, OpError> {
        unreachable!("{NOT_USED}")
    }

    fn list_fields(&self, _: &str, _: &str, _: &str) -> Result<Vec<&OpField>, OpError> {
        unreachable!("{NOT_USED}")
    }

    fn reference(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
        _: Option<&str>,
    ) -> Result<OpReference, OpError> {
        unreachable!("{NOT_USED}")
    }

    fn describe(&self, _: &OpReference) -> Result<FieldDescriptor, OpError> {
        unreachable!("{NOT_USED}")
    }

    fn endpoint_of(&self, _: &OpReference) -> Option<String> {
        None
    }

    fn classify(&self, _: &OpReference) -> Result<SecretClass, OpError> {
        unreachable!("{NOT_USED}")
    }

    fn box_clone(&self) -> Box<dyn OnePassword> {
        Box::new(self.clone())
    }
}
