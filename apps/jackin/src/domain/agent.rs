#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Agent {
    ClaudeCode,
    Codex,
    Amp,
    KimiCode,
    OpenCode,
    GrokBuild,
    MuseCode,
    Antigravity,
    MinimaxCode,
    MimoCode,
    Omp,
}

impl Agent {
    pub const ALL: [Agent; 11] = [
        Agent::ClaudeCode,
        Agent::Codex,
        Agent::Amp,
        Agent::KimiCode,
        Agent::OpenCode,
        Agent::GrokBuild,
        Agent::MuseCode,
        Agent::Antigravity,
        Agent::MinimaxCode,
        Agent::MimoCode,
        Agent::Omp,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "Claude Code",
            Agent::Codex => "Codex",
            Agent::Amp => "Amp",
            Agent::KimiCode => "Kimi Code",
            Agent::OpenCode => "OpenCode",
            Agent::GrokBuild => "Grok Build",
            Agent::MuseCode => "Muse",
            Agent::Antigravity => "Antigravity",
            Agent::MinimaxCode => "MiniMax Code",
            Agent::MimoCode => "Mimo Code",
            Agent::Omp => "OMP",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "claude",
            Agent::Codex => "codex",
            Agent::Amp => "amp",
            Agent::KimiCode => "kimi",
            Agent::OpenCode => "opencode",
            Agent::GrokBuild => "grok",
            Agent::MuseCode => "muse",
            Agent::Antigravity => "antigravity",
            Agent::MinimaxCode => "minimax",
            Agent::MimoCode => "mimo",
            Agent::Omp => "omp",
        }
    }

    pub fn provider(self) -> Provider {
        match self {
            Agent::ClaudeCode => Provider::Anthropic,
            Agent::Codex => Provider::OpenAi,
            Agent::Amp => Provider::Amp,
            Agent::KimiCode => Provider::Moonshot,
            Agent::OpenCode => Provider::OpenCode,
            Agent::GrokBuild => Provider::XAi,
            Agent::MuseCode => Provider::Muse,
            Agent::Antigravity => Provider::Antigravity,
            Agent::MinimaxCode => Provider::MiniMax,
            Agent::MimoCode => Provider::Mimo,
            Agent::Omp => Provider::OpenCode,
        }
    }

    pub fn auth_modes(self) -> &'static [AuthMode] {
        match self {
            Agent::ClaudeCode => &[
                AuthMode::Sync,
                AuthMode::ApiKey,
                AuthMode::OAuthToken,
                AuthMode::Ignore,
            ],
            _ => &[AuthMode::Sync, AuthMode::ApiKey, AuthMode::Ignore],
        }
    }

    pub fn registerable(self) -> bool {
        matches!(
            self,
            Agent::ClaudeCode | Agent::Codex | Agent::GrokBuild | Agent::OpenCode
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Provider {
    Anthropic,
    OpenAi,
    Amp,
    XAi,
    OpenCode,
    Moonshot,
    Zai,
    MiniMax,
    Cursor,
    Gemini,
    Openrouter,
    Antigravity,
    Hermes,
    Muse,
    Mimo,
}

impl Provider {
    pub const ALL: [Provider; 15] = [
        Provider::Anthropic,
        Provider::OpenAi,
        Provider::Amp,
        Provider::XAi,
        Provider::OpenCode,
        Provider::Moonshot,
        Provider::Zai,
        Provider::MiniMax,
        Provider::Cursor,
        Provider::Gemini,
        Provider::Openrouter,
        Provider::Antigravity,
        Provider::Hermes,
        Provider::Muse,
        Provider::Mimo,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic / Claude",
            Provider::OpenAi => "OpenAI",
            Provider::Amp => "Amp",
            Provider::XAi => "xAI / Grok",
            Provider::OpenCode => "OpenCode",
            Provider::Moonshot => "Moonshot / Kimi",
            Provider::Zai => "Z.AI",
            Provider::MiniMax => "MiniMax",
            Provider::Cursor => "Cursor",
            Provider::Gemini => "Google Gemini",
            Provider::Openrouter => "OpenRouter",
            Provider::Antigravity => "Antigravity",
            Provider::Hermes => "Hermes",
            Provider::Muse => "Muse",
            Provider::Mimo => "Mimo",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic",
            Provider::OpenAi => "OpenAI",
            Provider::Amp => "Amp",
            Provider::XAi => "xAI",
            Provider::OpenCode => "OpenCode",
            Provider::Moonshot => "Moonshot",
            Provider::Zai => "Z.AI",
            Provider::MiniMax => "MiniMax",
            Provider::Cursor => "Cursor",
            Provider::Gemini => "Gemini",
            Provider::Openrouter => "OpenRouter",
            Provider::Antigravity => "Antigravity",
            Provider::Hermes => "Hermes",
            Provider::Muse => "Muse",
            Provider::Mimo => "Mimo",
        }
    }

    pub fn usage_surface(self) -> UsageSurface {
        match self {
            Provider::Anthropic => UsageSurface::Claude,
            Provider::OpenAi => UsageSurface::Codex,
            Provider::Amp => UsageSurface::Amp,
            Provider::XAi => UsageSurface::Grok,
            Provider::OpenCode => UsageSurface::OpenCode,
            Provider::Moonshot => UsageSurface::Kimi,
            Provider::Zai => UsageSurface::Zai,
            Provider::MiniMax => UsageSurface::Minimax,
            Provider::Cursor
            | Provider::Gemini
            | Provider::Openrouter
            | Provider::Antigravity
            | Provider::Hermes
            | Provider::Mimo
            | Provider::Muse => UsageSurface::Unsupported,
        }
    }

    pub fn agent(self) -> Option<Agent> {
        match self {
            Provider::Anthropic => Some(Agent::ClaudeCode),
            Provider::OpenAi => Some(Agent::Codex),
            Provider::Amp => Some(Agent::Amp),
            Provider::XAi => Some(Agent::GrokBuild),
            Provider::OpenCode => Some(Agent::OpenCode),
            Provider::Moonshot => Some(Agent::KimiCode),
            Provider::Zai
            | Provider::MiniMax
            | Provider::Cursor
            | Provider::Gemini
            | Provider::Openrouter
            | Provider::Antigravity
            | Provider::Hermes
            | Provider::Muse => None,
            Provider::Mimo => Some(Agent::MimoCode),
        }
    }

    pub fn supports_endpoint(self) -> bool {
        matches!(self, Provider::XAi)
    }

    pub fn plain_key_label(self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic API key",
            Provider::OpenAi => "OpenAI API key",
            Provider::XAi => "xAI / deployment API key",
            Provider::OpenCode => "OpenCode API key",
            Provider::Amp => "Amp API key",
            Provider::Moonshot => "Kimi Code API key",
            Provider::Zai => "Z.AI API key",
            Provider::MiniMax => "MiniMax API key",
            Provider::Cursor => "Cursor API key",
            Provider::Gemini => "Gemini API key",
            Provider::Openrouter => "OpenRouter API key",
            Provider::Antigravity => "Antigravity API key",
            Provider::Hermes => "Hermes API key",
            Provider::Muse => "Muse API key",
            Provider::Mimo => "Mimo API key",
        }
    }

    pub fn folder_label(self) -> &'static str {
        match self {
            Provider::Anthropic => "Claude profile / home folder",
            Provider::OpenAi => "CODEX_HOME folder",
            Provider::XAi => "Grok profile folder",
            Provider::OpenCode => "OpenCode profile folder",
            Provider::Amp => "Amp profile folder",
            Provider::Moonshot => "Kimi profile folder",
            Provider::Zai => "Z.AI profile folder",
            Provider::MiniMax => "MiniMax profile folder",
            Provider::Cursor => "Cursor profile folder",
            Provider::Gemini => "Gemini profile folder",
            Provider::Openrouter => "OpenRouter profile folder",
            Provider::Antigravity => "Antigravity profile folder",
            Provider::Hermes => "Hermes profile folder",
            Provider::Muse => "Muse profile folder",
            Provider::Mimo => "Mimo profile folder",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UsageSurface {
    Claude,
    Codex,
    Amp,
    Grok,
    Zai,
    Kimi,
    Minimax,
    OpenCode,
    Unsupported,
}

impl UsageSurface {
    pub const ALL: [UsageSurface; 9] = [
        UsageSurface::Claude,
        UsageSurface::Codex,
        UsageSurface::Amp,
        UsageSurface::Grok,
        UsageSurface::Zai,
        UsageSurface::Kimi,
        UsageSurface::Minimax,
        UsageSurface::OpenCode,
        UsageSurface::Unsupported,
    ];

    pub fn label(self) -> &'static str {
        match self {
            UsageSurface::Claude => "Anthropic",
            UsageSurface::Codex => "OpenAI",
            UsageSurface::Amp => "Amp",
            UsageSurface::Grok => "xAI",
            UsageSurface::Zai => "Z.AI",
            UsageSurface::Kimi => "Kimi",
            UsageSurface::Minimax => "MiniMax",
            UsageSurface::OpenCode => "OpenCode",
            UsageSurface::Unsupported => "Usage",
        }
    }

    pub fn surface_name(self) -> &'static str {
        match self {
            UsageSurface::Claude => "Claude",
            UsageSurface::Codex => "Codex",
            UsageSurface::Amp => "Amp",
            UsageSurface::Grok => "Grok",
            UsageSurface::Zai => "Z.AI",
            UsageSurface::Kimi => "Kimi",
            UsageSurface::Minimax => "MiniMax",
            UsageSurface::OpenCode => "OpenCode",
            UsageSurface::Unsupported => "Unsupported",
        }
    }

    pub fn provider(self) -> Option<Provider> {
        match self {
            UsageSurface::Claude => Some(Provider::Anthropic),
            UsageSurface::Codex => Some(Provider::OpenAi),
            UsageSurface::Amp => Some(Provider::Amp),
            UsageSurface::Grok => Some(Provider::XAi),
            UsageSurface::Zai => Some(Provider::Zai),
            UsageSurface::Kimi => Some(Provider::Moonshot),
            UsageSurface::Minimax => Some(Provider::MiniMax),
            UsageSurface::OpenCode => Some(Provider::OpenCode),
            UsageSurface::Unsupported => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthMode {
    Sync,
    ApiKey,
    OAuthToken,

    Ignore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentBlocked {
    pub launchable: Agent,
    pub text: String,
}

impl AgentBlocked {
    pub fn openshell() -> Self {
        Self {
            launchable: Agent::Codex,
            text: "usage only · launches Codex only".into(),
        }
    }
}
