use std::sync::Arc;

use crate::console::domain::account::AccountId;
use crate::console::domain::account::{
    Account, AccountIdentity, Confidence, CredentialSource, DetectedKind, IssueCode, Lifecycle,
    RecoverableIssue, ValidationLevel,
};
use crate::console::domain::agent::{Agent, Provider};
use crate::console::domain::instance::InstanceId;
use crate::console::domain::onepassword::OpReference;
use crate::console::domain::role::RoleManifestDoc;
use crate::console::domain::usage::AccountUsage;
use crate::console::domain::workspace::{RoleName, WorkspaceId};
use crate::console::launch::LaunchContent;
use crate::console::panes::SessionScript;
use crate::console::world::{Msg, World};

pub use crate::widgets::diff::{DiffFile as ChangedFile, DiffHunk as Hunk, DiffLine, DiffStatus};
pub use crate::widgets::inspect_changes::ChangeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationOutcome {
    pub level: Option<ValidationLevel>,
    pub identity: AccountIdentity,
    pub confidence: Confidence,
    pub lifecycle: Lifecycle,
    pub issue: Option<RecoverableIssue>,
    pub usage: Option<AccountUsage>,

    pub material: CheckRow,
    pub identity_row: CheckRow,
    pub quota_row: CheckRow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckRow {
    Ok(String),
    Failed(String),
    Skipped(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderProbe {
    Missing,
    Unreadable,

    NoCredential,

    Malformed,
    Found(DetectedKind),
}

pub fn apply_validation(a: &mut Account, v: &ValidationOutcome, now: i64) {
    a.validation = match (&v.level, &v.issue) {
        (Some(l), None) => crate::console::domain::account::ValidationState::Valid(*l),
        (Some(l), Some(i))
            if i.is_informational()
                || i.code == IssueCode::QuotaUnsupported
                || i.code == IssueCode::RateLimited =>
        {
            crate::console::domain::account::ValidationState::Valid(*l)
        }
        (_, Some(i)) => crate::console::domain::account::ValidationState::Invalid(i.clone()),
        (None, None) => crate::console::domain::account::ValidationState::NeverValidated,
    };
    a.identity = v.identity.clone();
    a.confidence = v.confidence;
    a.lifecycle = v.lifecycle;
    a.issue = v.issue.clone();
    a.last_refresh_secs = Some(now);
    if let Some(u) = &v.usage {
        if u.windows.is_empty() {
            a.usage.freshness = u.freshness.clone();
        } else {
            a.usage = u.clone();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    Valid,
    Rejected,
    RateLimited,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretClass {
    Empty,
    Unrecognised,
    Key {
        provider: Provider,
        outcome: KeyOutcome,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpSession {
    SignedIn,
    Locked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpAccountState {
    Available,
    Locked,
    AuthorizationRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultAccess {
    ReadWrite,
    ReadOnly,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Concealed,
    Text,
    Url,
}

impl FieldKind {
    pub fn label(self) -> &'static str {
        match self {
            FieldKind::Concealed => "concealed",
            FieldKind::Text => "text",
            FieldKind::Url => "url",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpField {
    pub id: String,
    pub label: String,
    pub kind: FieldKind,

    pub material: Option<String>,

    pub tail: String,
}

#[derive(Debug, Clone)]
pub struct OpItem {
    pub id: String,
    pub title: String,
    pub category: &'static str,
    pub fields: Vec<OpField>,
}

#[derive(Debug, Clone)]
pub struct OpVault {
    pub id: String,
    pub name: String,
    pub access: VaultAccess,
    pub items: Vec<OpItem>,
}

#[derive(Debug, Clone)]
pub struct OpAccount {
    pub id: String,
    pub email: String,
    pub state: OpAccountState,
    pub vaults: Vec<OpVault>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpError {
    Locked,
    AuthorizationRequired { account: String },
    PermissionDenied { vault: String },
    MissingAccount { account: String },
    MissingVault { vault: String },
    MissingItem { item: String, vault: String },
    MissingField { field: String, item: String },
    EmptyMaterial { field: String },
    WrongFieldShape { field: String },
}

impl OpError {
    pub fn message(&self) -> String {
        match self {
            OpError::Locked => "1Password locked: unlock the app and retry".into(),
            OpError::AuthorizationRequired { account } => {
                format!("1Password authorization required: sign in to {account}")
            }
            OpError::PermissionDenied { vault } => {
                format!("1Password access denied: no permission for vault {vault}")
            }
            OpError::MissingAccount { account } => {
                format!("1Password account not found: {account}")
            }
            OpError::MissingVault { vault } => format!("1Password vault not found: {vault}"),
            OpError::MissingItem { item, vault } => {
                format!("1Password item not found: {item} in {vault}")
            }
            OpError::MissingField { field, item } => {
                format!("1Password field not found: {field} in {item}")
            }
            OpError::EmptyMaterial { field } => format!("API key required: field {field} is empty"),
            OpError::WrongFieldShape { field } => {
                format!("Field {field} is not a credential: choose a concealed field")
            }
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(self, OpError::Locked)
    }
}

#[derive(Debug, Clone)]
pub struct FieldDescriptor {
    pub masked: String,
}

pub trait OnePassword: std::fmt::Debug {
    fn session(&self) -> OpSession;

    fn set_session(&mut self, session: OpSession);

    fn accounts(&self) -> &[OpAccount];

    fn latency_ms(&self) -> i64;

    fn list_accounts(&self) -> Result<Vec<&OpAccount>, OpError>;

    fn list_vaults(&self, account: &str) -> Result<Vec<&OpVault>, OpError>;

    fn list_items(&self, account: &str, vault: &str) -> Result<Vec<&OpItem>, OpError>;

    fn list_fields(&self, account: &str, vault: &str, item: &str)
    -> Result<Vec<&OpField>, OpError>;

    fn reference(
        &self,
        account: &str,
        vault: &str,
        item: &str,
        field: &str,
    ) -> Result<OpReference, OpError>;

    fn describe(&self, reference: &OpReference) -> Result<FieldDescriptor, OpError>;

    fn endpoint_of(&self, reference: &OpReference) -> Option<String>;

    fn classify(&self, reference: &OpReference) -> Result<SecretClass, OpError>;

    fn box_clone(&self) -> Box<dyn OnePassword>;
}

impl Clone for Box<dyn OnePassword> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    SaveWorkspace {
        id: WorkspaceId,
    },
    SaveGlobal,
    Prewarm {
        workspace: WorkspaceId,
    },
    Stop {
        instance: InstanceId,
    },
    Purge {
        instance: InstanceId,
    },
    Refresh,
    RefreshAccount {
        account: AccountId,
        stagger_ms: i64,
    },
    ValidateAccount {
        account: AccountId,
    },
    LoadRole {
        role: String,
    },
    CheckRole {
        role: RoleName,
        doc: RoleManifestDoc,
    },
    SaveRole {
        role: RoleName,
        doc: RoleManifestDoc,
    },
    BindRole {
        role: RoleName,
        agent: Agent,
        account: Option<AccountId>,
    },
    AddRole {
        location: String,
    },
    RunDoctor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub after_ms: i64,
    pub msg: Msg,
}

pub trait Backend: std::fmt::Debug + Send + Sync {
    fn validate(
        &self,
        provider: Provider,
        source: &CredentialSource,
        plain_value: Option<&str>,
        op: &dyn OnePassword,
        now: i64,
    ) -> ValidationOutcome;

    fn probe_folder(&self, path: &str) -> FolderProbe;

    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion>;

    fn changes(
        &self,
        instance_id: &str,
        touched: &[String],
        uncommitted: usize,
        unpushed: usize,
    ) -> ChangeSet;

    fn launch_content(&self) -> Arc<LaunchContent>;

    fn session_script(&self) -> Arc<dyn SessionScript>;
}
