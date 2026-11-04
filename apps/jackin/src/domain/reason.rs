//! The facts behind what the console tells the operator about an account's
//! credential and usage. A backend names the case and its facts; the
//! `Vocabulary` (`ReasonWords`) words it. No backend writes a sentence.

use super::account::{DetectedKind, ValidationLevel};
use super::agent::Provider;
use crate::backend::OpError;

/// Why a credential check, a validation or a refresh did not succeed, or was
/// not made. The same value is worded twice: as the line of an issue
/// (`Vocabulary::reason`) and as the text of a check row
/// (`Vocabulary::check`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueReason {
    /// A check was not made because an earlier one failed.
    Skipped,
    /// 1Password: locked, not signed in, not available, no access, or the
    /// account, vault, item or field was not found or is not usable.
    OnePassword(OpError),
    EnvInvalid {
        name: String,
    },
    EnvMissing {
        name: String,
    },
    EnvEmpty {
        name: String,
    },
    /// The text is not a 1Password field reference.
    BadReference {
        reference: String,
    },
    FileMissing {
        path: String,
    },
    FileNotRegular {
        path: String,
    },
    FolderMissing {
        path: String,
    },
    /// The login folder cannot be read; `detail` is what the host reported.
    FolderUnreadable {
        path: String,
        detail: String,
    },
    /// The folder holds no login: `file` is the one it lacks.
    LoginMissing {
        path: String,
        file: String,
    },
    /// The login file is not valid.
    LoginInvalid {
        file: String,
    },
    /// The folder holds the login of another provider; `found` is `None`
    /// when it holds none the console knows.
    WrongProvider {
        found: Option<Provider>,
        expected: Provider,
    },
    KeyEmpty,
    KeyRejected,
    KeyRevoked,
    KeyWrongProvider {
        found: Provider,
        expected: Provider,
    },
    /// Usage could not be read, for one of the reasons a provider reports.
    Usage(UsageReason),
    QuotaHidden {
        provider: Provider,
    },
    QuotaDenied {
        provider: Provider,
        status: u16,
    },
    Rejected {
        provider: Provider,
    },
    ProviderDown {
        provider: Provider,
        detail: String,
    },
    BrokerDown,
    /// The last good reading is `age_secs` old.
    Stale {
        age_secs: i64,
    },
    IdentityUnresolved,
    PresenceOnly,
}

/// Why a provider gave no usage (D25).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsageReason {
    NoCredentials,
    ExpiredLogin,
    Unsupported,
    RateLimited { retry_in_secs: Option<i64> },
    Network { detail: String },
    Provider { detail: String },
}

/// What a check found, for the row of a check that passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    ReferenceResolved,
    KeyPresent,
    Detected(DetectedKind),
    Unparsable { file: String, line: u32 },
    Authenticated { who: String, plan: Option<String> },
    WindowsReadable(usize),
    EnvSet { name: String },
    FileFound { path: String },
    FolderFound { path: String },
    Login { account: String },
    Level(ValidationLevel),
}

/// How a refresh of one account ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshOutcome {
    Current,
    Stale,
    Failed,
    StillRateLimited,
    ProviderUnavailable,
    NeedsAttention,
    BrokerUnreachable,
    OpLocked,
}
