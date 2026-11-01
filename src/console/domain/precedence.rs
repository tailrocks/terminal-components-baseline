use crate::console::domain::account::*;
use crate::console::domain::agent::Provider;
use crate::console::domain::workspace::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrecedenceLevel {
    Session,
    Role,
    Workspace,
    ProviderDefault,
    Discovered,
    None,
}

impl PrecedenceLevel {
    pub fn label(self) -> &'static str {
        match self {
            PrecedenceLevel::Session => "session choice",
            PrecedenceLevel::Role => "Role choice",
            PrecedenceLevel::Workspace => "Workspace choice",
            PrecedenceLevel::ProviderDefault => "provider default",
            PrecedenceLevel::Discovered => "discovered on host",
            PrecedenceLevel::None => "no account",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAccount {
    pub account: Option<AccountId>,
    pub level: PrecedenceLevel,
    pub why: String,

    pub shadowed: Vec<(PrecedenceLevel, AccountId)>,
}

impl ResolvedAccount {
    pub fn label(&self, registry: &AccountRegistry) -> String {
        match &self.account {
            Some(id) => registry
                .get(id)
                .map(|a| a.title())
                .unwrap_or_else(|| id.clone()),
            None => "no account".into(),
        }
    }
}

pub fn resolve_account(
    provider: Provider,
    ws: Option<&Workspace>,
    role: Option<&str>,
    session: Option<&AccountId>,
    registry: &AccountRegistry,
) -> ResolvedAccount {
    let effective = ws.map(|w| w.effective_accounts(registry));
    let in_set = |id: &AccountId| -> bool {
        effective
            .as_ref()
            .is_none_or(|e| e.iter().any(|x| &x.id == id && x.provider == provider))
    };
    let mut candidates: Vec<(PrecedenceLevel, AccountId, String)> = vec![];
    if let Some(s) = session
        && in_set(s)
    {
        candidates.push((
            PrecedenceLevel::Session,
            s.clone(),
            "Session choice · picked for this session in Capsule".into(),
        ));
    }
    if let (Some(w), Some(r)) = (ws, role)
        && let Some(id) = w.role_preference(r, provider, registry)
    {
        candidates.push((
            PrecedenceLevel::Role,
            id,
            format!("Role preference · set on Role {r} in {}", w.name),
        ));
    }
    if let Some(w) = ws
        && let Some(id) = w.accounts.preferred.get(&provider)
        && in_set(id)
    {
        candidates.push((
            PrecedenceLevel::Workspace,
            id.clone(),
            format!("Workspace preference · set in {} › Accounts", w.name),
        ));
    }
    if let Some(a) = registry.default_for(provider)
        && in_set(&a.id)
    {
        candidates.push((
            PrecedenceLevel::ProviderDefault,
            a.id.clone(),
            "Provider default · set in Account & Usage Center".into(),
        ));
    }
    if let Some(a) = registry.discovered_current(provider)
        && in_set(&a.id)
    {
        candidates.push((
            PrecedenceLevel::Discovered,
            a.id.clone(),
            format!(
                "Discovered on host · no explicit choice for {}",
                provider.short()
            ),
        ));
    }
    if let (Some(w), Some(set)) = (ws, effective.as_ref()) {
        let extra: Vec<AccountId> = set
            .iter()
            .filter(|e| e.provider == provider && !candidates.iter().any(|(_, id, _)| id == &e.id))
            .map(|e| e.id.clone())
            .collect();
        for id in extra {
            candidates.push((
                PrecedenceLevel::Workspace,
                id,
                format!("Enabled in {} › Accounts", w.name),
            ));
        }
    }
    let mut shadowed = vec![];
    let mut chosen: Option<(PrecedenceLevel, AccountId, String)> = None;
    for (level, id, why) in candidates {
        let usable = registry.get(&id).is_some_and(|a| a.enabled);
        if chosen.is_none() {
            if usable {
                chosen = Some((level, id, why));
            } else {
                shadowed.push((level, format!("{id} (disabled)")));
            }
        } else {
            shadowed.push((level, id));
        }
    }
    match chosen {
        Some((level, id, mut why)) => {
            if let Some((l, other)) = shadowed.first() {
                let other_label = registry
                    .get(other.trim_end_matches(" (disabled)"))
                    .map(|a| a.display_name.clone())
                    .unwrap_or_else(|| other.clone());
                if other.ends_with("(disabled)") {
                    why.push_str(&format!(" · {} {other_label} skipped: disabled", l.label()));
                } else {
                    why.push_str(&format!(" · overrides {} {other_label}", l.label()));
                }
            }
            ResolvedAccount {
                account: Some(id),
                level,
                why,
                shadowed,
            }
        }
        None => ResolvedAccount {
            account: None,
            level: PrecedenceLevel::None,
            why: match ws {
                Some(w) => format!(
                    "No account: enable one in {} › Accounts or register one in Account & Usage Center",
                    w.name
                ),
                None => "No account: register one in Account & Usage Center".into(),
            },
            shadowed,
        },
    }
}
