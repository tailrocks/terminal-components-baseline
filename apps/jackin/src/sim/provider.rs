use super::onepassword::classify_plain;
use crate::backend::{
    CheckRow, FolderProbe, KeyOutcome, OnePassword, OpAccountRow, OpError, OpFieldRow, OpItemRow,
    OpListing, OpScope, OpVaultRow, Refresh, SecretClass, ValidationOutcome,
};
use crate::domain::account::{
    Account, AccountIdentity, Confidence, CredentialSource, DetectedKind, IdentitySubject,
    IssueCode, Lifecycle, Recoverability, RecoverableIssue, ValidationLevel,
};
use crate::domain::agent::Provider;
use crate::domain::reason::{Found, IssueReason, RefreshOutcome, UsageReason};
use crate::domain::usage::{
    AccountUsage, Freshness, FreshnessInfo, QuotaStatus, QuotaWindow, WindowCategory, WindowUnit,
};

/// How long the simulated provider takes to check a source. A folder and a
/// typed key are checked on the spot, as the form always did; a 1Password
/// reference goes through `op` and then to the provider.
pub fn check_duration_ms(source: &CredentialSource) -> i64 {
    match source {
        CredentialSource::OnePassword(_) => 900,
        CredentialSource::LocalFolder { .. }
        | CredentialSource::HostEnv { .. }
        | CredentialSource::PlainApiKey { .. } => 0,
    }
}

/// What the simulated 1Password lists for one step of the picker.
pub fn list_op(op: &dyn OnePassword, scope: &OpScope) -> Result<OpListing, OpError> {
    Ok(match scope {
        OpScope::Accounts => OpListing::Accounts(
            op.list_accounts()?
                .into_iter()
                .map(|a| OpAccountRow {
                    id: a.id.clone(),
                    email: a.email.clone(),
                    state: a.state,
                })
                .collect(),
        ),
        OpScope::Vaults { account } => OpListing::Vaults(
            op.list_vaults(account)?
                .into_iter()
                .map(|v| OpVaultRow {
                    id: v.id.clone(),
                    name: v.name.clone(),
                    items: v.items.len(),
                    access: Some(v.access),
                })
                .collect(),
        ),
        OpScope::Items { account, vault } => OpListing::Items(
            op.list_items(account, vault)?
                .into_iter()
                .map(|i| OpItemRow {
                    id: i.id.clone(),
                    title: i.title.clone(),
                    category: i.category.to_owned(),
                    fields: Some(i.fields.len()),
                })
                .collect(),
        ),
        OpScope::Fields {
            account,
            vault,
            item,
        } => OpListing::Fields(
            op.list_fields(account, vault, item)?
                .into_iter()
                .map(|f| OpFieldRow {
                    id: f.id.clone(),
                    label: f.label.clone(),
                    kind: f.kind,
                })
                .collect(),
        ),
    })
}

pub fn probe_folder(path: &str) -> FolderProbe {
    let p = path.trim_end_matches('/');
    match p {
        "~/.claude" | "/Users/alexey/.claude" => {
            FolderProbe::Found(DetectedKind::ClaudeOAuthProfile)
        }
        "~/.claude-work" => FolderProbe::Found(DetectedKind::ClaudeApiKeyEnv),
        "~/.codex" | "/Users/alexey/.codex" => FolderProbe::Found(DetectedKind::CodexAuthJson),
        "~/.grok" => FolderProbe::NoCredential,
        "~/.grok-team" => FolderProbe::Found(DetectedKind::GrokAuthJson),
        "~/.local/share/opencode" | "~/.local/share/opencode/auth.json" => {
            FolderProbe::Found(DetectedKind::OpenCodeGoAuthJson)
        }
        "~/.opencode-broken" => FolderProbe::Malformed,
        "~/.codex-locked" => FolderProbe::Unreadable,
        "~/.kimi" => FolderProbe::Found(DetectedKind::KimiApiKeyEnv),
        "~/.config/amp" => FolderProbe::Found(DetectedKind::AmpSecrets),
        "~/Library/Application Support/jackin/profiles/claude-contractor-2025-archive" => {
            FolderProbe::Found(DetectedKind::ClaudeApiKeyEnv)
        }
        _ => FolderProbe::Missing,
    }
}

fn issue(code: IssueCode, reason: IssueReason, rec: Recoverability) -> RecoverableIssue {
    RecoverableIssue::new(code, reason, rec)
}

fn not_attempted() -> CheckRow {
    CheckRow::Skipped(IssueReason::Skipped)
}

fn identity_for(provider: Provider, tag: &str) -> (AccountIdentity, Confidence) {
    let (subject, plan) = match (provider, tag) {
        (Provider::Anthropic, t) if t.contains("ant02") => ("alexey@donbeave.dev", "Max 5x"),
        (Provider::Anthropic, _) => ("alexey@chainargos.com", "Team"),
        (Provider::OpenAi, t) if t.contains("cdx02") => ("ChatGPT account org_7Hq2", "Plus"),
        (Provider::OpenAi, _) => ("ChatGPT account org_7Hq2", "Pro 20x"),
        (Provider::XAi, _) => ("team_chainargos", "Team · prepaid"),
        (Provider::OpenCode, _) => ("donbeave", "OpenCode Go"),
        _ => ("unknown", "unknown"),
    };
    (
        AccountIdentity {
            subject: Some(IdentitySubject::Handle(subject.into())),
            plan: Some(plan.into()),
        },
        Confidence::Authoritative,
    )
}

pub fn windows_for(provider: Provider, now: i64, key_backed: bool) -> Vec<QuotaWindow> {
    let h = 3600;
    match provider {
        Provider::Anthropic => vec![
            QuotaWindow::pct("session", "Session · 5-hour", WindowCategory::Session, 38)
                .reset(now + 3 * h + 12 * 60),
            QuotaWindow::pct(
                "weekly",
                "Weekly · all models",
                WindowCategory::LongRange,
                33,
            )
            .reset(now + 3 * 86_400 + 10 * h),
            QuotaWindow::pct(
                "weekly_sonnet",
                "Weekly · Sonnet",
                WindowCategory::Model,
                21,
            )
            .reset(now + 3 * 86_400 + 10 * h),
            QuotaWindow::pct("weekly_opus", "Weekly · Opus", WindowCategory::Model, 54)
                .reset(now + 3 * 86_400 + 10 * h),
        ],
        Provider::OpenAi if key_backed => vec![QuotaWindow::sentinel(
            "quota",
            "Quota",
            QuotaStatus::Unsupported,
            "Quota not visible for API keys",
        )],
        Provider::OpenAi => vec![
            QuotaWindow::pct("session", "Session · 5-hour", WindowCategory::Session, 12)
                .reset(now + 4 * h + 40 * 60),
            QuotaWindow::pct("weekly", "Weekly · 7-day", WindowCategory::LongRange, 59)
                .reset(now + 2 * 86_400 + 19 * h),
            QuotaWindow::not_started("spark", "Codex Spark · 5-hour", WindowCategory::Model),
            QuotaWindow::counted(
                "credits",
                "Credits",
                WindowCategory::Other,
                WindowUnit::Credits,
                1_240,
                5_000,
            ),
        ],
        Provider::XAi => vec![
            QuotaWindow::pct("monthly", "Monthly", WindowCategory::LongRange, 31)
                .reset(now + 27 * 86_400 + 10 * h),
            QuotaWindow::pct("weekly", "Weekly", WindowCategory::LongRange, 68)
                .reset(now + 3 * 86_400 + 10 * h),
            QuotaWindow::counted(
                "credits",
                "Credits",
                WindowCategory::Other,
                WindowUnit::Usd,
                3_140,
                10_000,
            )
            .spend("$68.60 remaining of $100.00"),
            QuotaWindow::counted(
                "ondemand",
                "On-demand usage",
                WindowCategory::Other,
                WindowUnit::Usd,
                315,
                315,
            )
            .status(QuotaStatus::Available)
            .spend("$3.15 this month"),
        ],
        Provider::OpenCode => vec![
            QuotaWindow::pct("rolling", "Rolling", WindowCategory::Session, 57)
                .reset(now + h + 50 * 60),
            QuotaWindow::pct("weekly", "Weekly", WindowCategory::LongRange, 45)
                .reset(now + 3 * 86_400),
            QuotaWindow::pct("monthly", "Monthly", WindowCategory::LongRange, 22)
                .reset(now + 27 * 86_400),
        ],
        _ => vec![],
    }
}

pub fn validate(
    provider: Provider,
    source: &CredentialSource,
    plain_value: Option<&str>,
    op: &dyn OnePassword,
    now: i64,
) -> ValidationOutcome {
    let key_class: Result<(SecretClass, Found), OpError> = match source {
        CredentialSource::OnePassword(r) => match op.classify(r) {
            Ok(c) => Ok((c, Found::ReferenceResolved)),
            Err(e) => Err(e),
        },
        CredentialSource::PlainApiKey { .. } => Ok((
            classify_plain(provider, plain_value.unwrap_or("")),
            Found::KeyPresent,
        )),
        CredentialSource::LocalFolder { path, .. }
        | CredentialSource::HostEnv { var: path, .. } => {
            return validate_folder(provider, path, now);
        }
    };
    let (class, material_found) = match key_class {
        Ok(v) => v,
        Err(e) => {
            let code = match &e {
                OpError::Locked => IssueCode::OpLocked,
                OpError::NotAvailable => IssueCode::ProviderUnavailable,
                OpError::AuthorizationRequired { .. } => IssueCode::OpAuthorizationRequired,
                OpError::PermissionDenied { .. } => IssueCode::OpPermissionDenied,
                OpError::MissingAccount { .. }
                | OpError::MissingVault { .. }
                | OpError::MissingItem { .. } => IssueCode::OpItemMissing,
                OpError::MissingField { .. } | OpError::WrongFieldShape { .. } => {
                    IssueCode::OpFieldMissing
                }
                OpError::EmptyMaterial { .. } => IssueCode::ApiKeyEmpty,
            };
            let rec = if e.retryable() {
                Recoverability::Retryable
            } else {
                Recoverability::ActionRequired
            };
            return ValidationOutcome {
                level: None,
                identity: AccountIdentity::default(),
                confidence: Confidence::None,
                lifecycle: Lifecycle::NeedsSecret,
                issue: Some(issue(code, IssueReason::OnePassword(e.clone()), rec)),
                usage: None,
                material: CheckRow::Failed(IssueReason::OnePassword(e)),
                identity_row: not_attempted(),
                quota_row: not_attempted(),
            };
        }
    };
    match class {
        SecretClass::Empty => ValidationOutcome {
            level: None,
            identity: AccountIdentity::default(),
            confidence: Confidence::None,
            lifecycle: Lifecycle::NeedsSecret,
            issue: Some(issue(
                IssueCode::ApiKeyEmpty,
                IssueReason::KeyEmpty,
                Recoverability::ActionRequired,
            )),
            usage: None,
            material: CheckRow::Failed(IssueReason::KeyEmpty),
            identity_row: not_attempted(),
            quota_row: not_attempted(),
        },
        SecretClass::Unrecognised => ValidationOutcome {
            level: Some(ValidationLevel::MaterialDiscovered),
            identity: AccountIdentity::default(),
            confidence: Confidence::None,
            lifecycle: Lifecycle::Error,
            issue: Some(issue(
                IssueCode::ApiKeyInvalid,
                IssueReason::KeyRejected,
                Recoverability::ActionRequired,
            )),
            usage: None,
            material: CheckRow::Ok(material_found),
            identity_row: CheckRow::Failed(IssueReason::KeyRejected),
            quota_row: not_attempted(),
        },
        SecretClass::Key {
            provider: found,
            outcome,
        } => {
            if found != provider {
                return ValidationOutcome {
                    level: Some(ValidationLevel::MaterialDiscovered),
                    identity: AccountIdentity::default(),
                    confidence: Confidence::None,
                    lifecycle: Lifecycle::Error,
                    issue: Some(issue(
                        IssueCode::OpProviderMismatch,
                        IssueReason::KeyWrongProvider {
                            found,
                            expected: provider,
                        },
                        Recoverability::ActionRequired,
                    )),
                    usage: None,
                    material: CheckRow::Ok(material_found),
                    identity_row: CheckRow::Failed(IssueReason::KeyWrongProvider {
                        found,
                        expected: provider,
                    }),
                    quota_row: not_attempted(),
                };
            }
            let key_backed = true;
            let tag = match source {
                CredentialSource::OnePassword(r) => r.item_id.clone(),
                _ => plain_value.unwrap_or("").to_owned(),
            };
            let (identity, confidence) = identity_for(provider, &tag);
            match outcome {
                KeyOutcome::Valid => {
                    let windows = windows_for(provider, now, key_backed);
                    let unsupported = windows.iter().all(|w| w.status == QuotaStatus::Unsupported);
                    ValidationOutcome {
                        level: Some(if unsupported {
                            ValidationLevel::IdentityAuthenticated
                        } else {
                            ValidationLevel::QuotaReadable
                        }),
                        identity: identity.clone(),
                        confidence,
                        lifecycle: Lifecycle::Available,
                        issue: unsupported.then(|| {
                            issue(
                                IssueCode::QuotaUnsupported,
                                IssueReason::QuotaHidden { provider },
                                Recoverability::Unsupported,
                            )
                        }),
                        usage: Some(AccountUsage {
                            freshness: FreshnessInfo::current(now),
                            windows: windows.clone(),
                        }),
                        material: CheckRow::Ok(material_found),
                        identity_row: CheckRow::Ok(Found::Authenticated {
                            who: identity.label(),
                            plan: Some(identity.plan.clone().unwrap_or_default()),
                        }),
                        quota_row: if unsupported {
                            CheckRow::Skipped(IssueReason::QuotaHidden { provider })
                        } else {
                            CheckRow::Ok(Found::WindowsReadable(windows.len()))
                        },
                    }
                }
                KeyOutcome::Rejected => ValidationOutcome {
                    level: Some(ValidationLevel::MaterialDiscovered),
                    identity: AccountIdentity::default(),
                    confidence: Confidence::None,
                    lifecycle: Lifecycle::NeedsLogin,
                    issue: Some(issue(
                        IssueCode::ApiKeyInvalid,
                        IssueReason::KeyRevoked,
                        Recoverability::ActionRequired,
                    )),
                    usage: None,
                    material: CheckRow::Ok(material_found),
                    identity_row: CheckRow::Failed(IssueReason::KeyRevoked),
                    quota_row: not_attempted(),
                },
                KeyOutcome::RateLimited => ValidationOutcome {
                    level: Some(ValidationLevel::IdentityAuthenticated),
                    identity: identity.clone(),
                    confidence,
                    lifecycle: Lifecycle::Available,
                    issue: Some(
                        issue(
                            IssueCode::RateLimited,
                            IssueReason::Usage(UsageReason::RateLimited {
                                retry_in_secs: Some(25 * 60),
                            }),
                            Recoverability::Retryable,
                        )
                        .retry(now + 25 * 60),
                    ),
                    usage: Some(AccountUsage {
                        freshness: FreshnessInfo::failed(None, Some(now + 25 * 60)),
                        windows: vec![],
                    }),
                    material: CheckRow::Ok(material_found),
                    identity_row: CheckRow::Ok(Found::Authenticated {
                        who: identity.label(),
                        plan: None,
                    }),
                    quota_row: CheckRow::Failed(IssueReason::Usage(UsageReason::RateLimited {
                        retry_in_secs: Some(25 * 60),
                    })),
                },
                KeyOutcome::Unavailable => ValidationOutcome {
                    level: Some(ValidationLevel::MaterialDiscovered),
                    identity: AccountIdentity::default(),
                    confidence: Confidence::None,
                    lifecycle: Lifecycle::Unavailable,
                    issue: Some(issue(
                        IssueCode::ProviderUnavailable,
                        IssueReason::ProviderDown {
                            provider,
                            detail: "Timed out after 8 s".into(),
                        },
                        Recoverability::Retryable,
                    )),
                    usage: None,
                    material: CheckRow::Ok(material_found),
                    identity_row: CheckRow::Failed(IssueReason::ProviderDown {
                        provider,
                        detail: "Timed out after 8 s".into(),
                    }),
                    quota_row: not_attempted(),
                },
            }
        }
    }
}

fn unreadable(path: &str) -> IssueReason {
    IssueReason::FolderUnreadable {
        path: path.into(),
        detail: "mode 000 · owner root".into(),
    }
}

fn no_login(path: &str) -> IssueReason {
    IssueReason::LoginMissing {
        path: path.into(),
        file: "auth.json".into(),
    }
}

fn validate_folder(provider: Provider, path: &str, now: i64) -> ValidationOutcome {
    let fail = |code, reason: IssueReason, lifecycle, material: CheckRow| ValidationOutcome {
        level: if matches!(material, CheckRow::Ok(_)) {
            Some(ValidationLevel::MaterialDiscovered)
        } else {
            None
        },
        identity: AccountIdentity::default(),
        confidence: Confidence::None,
        lifecycle,
        issue: Some(issue(code, reason, Recoverability::ActionRequired)),
        usage: None,
        material,
        identity_row: not_attempted(),
        quota_row: not_attempted(),
    };
    match probe_folder(path) {
        FolderProbe::Missing => fail(
            IssueCode::FolderMissing,
            IssueReason::FolderMissing { path: path.into() },
            Lifecycle::Error,
            CheckRow::Failed(IssueReason::FolderMissing { path: path.into() }),
        ),
        FolderProbe::Unreadable => fail(
            IssueCode::FolderUnreadable,
            unreadable(path),
            Lifecycle::Error,
            CheckRow::Failed(unreadable(path)),
        ),
        FolderProbe::NoCredential => fail(
            IssueCode::CredentialFileMissing,
            no_login(path),
            Lifecycle::NeedsLogin,
            CheckRow::Failed(no_login(path)),
        ),
        FolderProbe::Malformed => fail(
            IssueCode::CredentialMalformed,
            IssueReason::LoginInvalid {
                file: "auth.json".into(),
            },
            Lifecycle::Error,
            CheckRow::Ok(Found::Unparsable {
                file: "auth.json".into(),
                line: 1,
            }),
        ),
        FolderProbe::Found(kind) => {
            if kind.provider() != Some(provider) {
                return fail(
                    IssueCode::FolderWrongProvider,
                    IssueReason::WrongProvider {
                        found: kind.provider(),
                        expected: provider,
                    },
                    Lifecycle::Error,
                    CheckRow::Ok(Found::Detected(kind)),
                );
            }
            let presence_only = matches!(
                kind,
                DetectedKind::ClaudeApiKeyEnv | DetectedKind::KimiApiKeyEnv
            );
            if presence_only {
                return ValidationOutcome {
                    level: Some(ValidationLevel::MaterialDiscovered),
                    identity: AccountIdentity::default(),
                    confidence: Confidence::PresenceOnly,

                    lifecycle: Lifecycle::Available,
                    issue: Some(issue(
                        IssueCode::IdentityUnresolved,
                        IssueReason::IdentityUnresolved,
                        Recoverability::Unsupported,
                    )),
                    usage: None,
                    material: CheckRow::Ok(Found::Detected(kind)),
                    identity_row: CheckRow::Skipped(IssueReason::IdentityUnresolved),
                    quota_row: CheckRow::Skipped(IssueReason::PresenceOnly),
                };
            }
            let (identity, _) = identity_for(provider, path);
            let identity = match provider {
                Provider::Anthropic => AccountIdentity {
                    subject: Some(IdentitySubject::Handle("alexey@donbeave.dev".into())),
                    plan: Some("Max 5x".into()),
                },
                _ => identity,
            };
            let windows = windows_for(provider, now, false);
            ValidationOutcome {
                level: Some(ValidationLevel::QuotaReadable),
                identity: identity.clone(),
                confidence: Confidence::Authoritative,
                lifecycle: Lifecycle::Available,
                issue: None,
                usage: Some(AccountUsage {
                    freshness: FreshnessInfo::current(now),
                    windows: windows.clone(),
                }),
                material: CheckRow::Ok(Found::Detected(kind)),
                identity_row: CheckRow::Ok(Found::Authenticated {
                    who: identity.label(),
                    plan: Some(identity.plan.clone().unwrap_or_default()),
                }),
                quota_row: CheckRow::Ok(Found::WindowsReadable(windows.len())),
            }
        }
    }
}

pub fn refresh_duration_ms(a: &Account) -> i64 {
    let base = match a.provider {
        Provider::Anthropic => 800,
        Provider::OpenAi => 1_000,
        Provider::XAi => 900,
        Provider::OpenCode => 600,
        Provider::Amp => 650,
        Provider::Zai => 1_050,
        Provider::Moonshot => 350,
        Provider::MiniMax => 1_600,
        Provider::Cursor
        | Provider::Gemini
        | Provider::Openrouter
        | Provider::Antigravity
        | Provider::Hermes
        | Provider::Muse => 700,
    };
    base + (a.id.len() as i64 % 5) * 40
}

/// What refreshing `account` finds. The broker being down keeps the last good
/// values, a locked 1Password fails the refresh, a rate limit holds until its
/// retry time, and a healthy account reads one point higher on its first meter.
pub fn refresh(account: &Account, now: i64, op_locked: bool, broker_down: bool) -> Refresh {
    let mut a = account.clone();
    let outcome: RefreshOutcome;
    match (&a.source, a.issue.as_ref().map(|i| i.code)) {
        _ if broker_down => {
            a.usage.freshness.phase = if a.usage.freshness.last_good_secs.is_some() {
                Freshness::Stale
            } else {
                Freshness::Failed
            };
            a.issue = Some(issue(
                IssueCode::ProviderUnavailable,
                IssueReason::BrokerDown,
                Recoverability::Retryable,
            ));
            outcome = RefreshOutcome::BrokerUnreachable;
        }
        (CredentialSource::OnePassword(_), _) if op_locked => {
            a.usage.freshness.phase = Freshness::Failed;
            a.issue = Some(issue(
                IssueCode::OpLocked,
                IssueReason::OnePassword(OpError::Locked),
                Recoverability::Retryable,
            ));
            outcome = RefreshOutcome::OpLocked;
        }
        (_, Some(IssueCode::RateLimited)) => {
            let retry = a.issue.as_ref().and_then(|i| i.retry_secs).unwrap_or(now);
            if now >= retry {
                a.usage.freshness = FreshnessInfo::current(now);
                a.issue = None;
                outcome = RefreshOutcome::Current;
            } else {
                a.usage.freshness.phase = Freshness::Failed;
                outcome = RefreshOutcome::StillRateLimited;
            }
        }
        (_, Some(IssueCode::ProviderUnavailable)) => {
            a.usage.freshness.phase = Freshness::Failed;
            outcome = RefreshOutcome::ProviderUnavailable;
        }
        (_, Some(IssueCode::CredentialFileMissing))
        | (_, Some(IssueCode::Unauthorized))
        | (_, Some(IssueCode::ApiKeyInvalid)) => {
            a.usage.freshness.phase = Freshness::Failed;
            outcome = RefreshOutcome::NeedsAttention;
        }
        _ => {
            a.usage.freshness = FreshnessInfo::current(now);
            if let Some(i) = &a.issue
                && i.code == IssueCode::Stale
            {
                a.issue = None;
            }
            if let Some(win) = a
                .usage
                .windows
                .iter_mut()
                .find(|w| w.used_pct.is_some() && w.status != QuotaStatus::Exhausted)
            {
                let p = (win.used_pct.unwrap_or(0) + 1).min(100);
                win.used_pct = Some(p);
                win.status = QuotaStatus::from_pct(p);
            }
            outcome = RefreshOutcome::Current;
        }
    }
    Refresh {
        usage: a.usage,
        issue: a.issue,
        last_refresh_secs: now,
        outcome,
    }
}
