use super::*;

pub(super) type ExposeDigest = [u8; 32];

pub(super) struct ExposeSessionStore {
    bootstrap: ExposeBootstrap,
    sessions: BTreeMap<ExposeDigest, ExposeSession>,
    max_sessions: usize,
}

impl fmt::Debug for ExposeSessionStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExposeSessionStore")
            .field("bootstrap", &"<redacted>")
            .field("sessions", &self.sessions.len())
            .field("max_sessions", &self.max_sessions)
            .finish()
    }
}

pub(super) struct ExposeBootstrap {
    digest: ExposeDigest,
    expires_at: Instant,
    consumed: bool,
}

pub(super) struct ExposeSession {
    csrf_digest: ExposeDigest,
    expires_at: Instant,
    last_seen: Instant,
    rate_window_started: Instant,
    rate_bytes: usize,
    rate_requests: usize,
}

pub(super) struct ExposeIssuedSession {
    pub(super) session: String,
    pub(super) csrf: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExposeSessionError {
    Unauthorized,
    Full,
    RateLimited,
    Internal,
}

impl ExposeSessionStore {
    pub(super) fn new(bootstrap: &str, max_sessions: usize, now: Instant) -> Self {
        Self {
            bootstrap: ExposeBootstrap {
                digest: expose_token_digest(bootstrap),
                expires_at: now + EXPOSE_BOOTSTRAP_TTL,
                consumed: false,
            },
            sessions: BTreeMap::new(),
            max_sessions: max_sessions.clamp(1, EXPOSE_MAX_CLIENTS_LIMIT),
        }
    }

    pub(super) fn exchange_bootstrap(
        &mut self,
        bootstrap: &str,
        now: Instant,
    ) -> std::result::Result<ExposeIssuedSession, ExposeSessionError> {
        self.prune(now);
        if self.bootstrap.consumed
            || now >= self.bootstrap.expires_at
            || !expose_digest_eq(&self.bootstrap.digest, &expose_token_digest(bootstrap))
        {
            return Err(ExposeSessionError::Unauthorized);
        }
        if self.sessions.len() >= self.max_sessions {
            return Err(ExposeSessionError::Full);
        }
        let issued = self.issue(now)?;
        self.bootstrap.consumed = true;
        Ok(issued)
    }

    pub(super) fn rotate(
        &mut self,
        session: &str,
        csrf: &str,
        now: Instant,
    ) -> std::result::Result<ExposeIssuedSession, ExposeSessionError> {
        let session_digest = expose_token_digest(session);
        if !self.authenticate(session, Some(csrf), now, false) {
            return Err(ExposeSessionError::Unauthorized);
        }
        let issued = self.issue(now)?;
        self.sessions.remove(&session_digest);
        Ok(issued)
    }

    pub(super) fn revoke(&mut self, session: &str, csrf: &str, now: Instant) -> bool {
        let digest = expose_token_digest(session);
        if !self.authenticate(session, Some(csrf), now, false) {
            return false;
        }
        self.sessions.remove(&digest).is_some()
    }

    pub(super) fn authenticate(
        &mut self,
        session: &str,
        csrf: Option<&str>,
        now: Instant,
        touch: bool,
    ) -> bool {
        self.prune(now);
        let Some(stored) = self.sessions.get_mut(&expose_token_digest(session)) else {
            return false;
        };
        if let Some(csrf) = csrf
            && !expose_digest_eq(&stored.csrf_digest, &expose_token_digest(csrf))
        {
            return false;
        }
        if touch {
            stored.last_seen = now;
        }
        true
    }

    pub(super) fn admit_input(
        &mut self,
        session: &str,
        csrf: &str,
        bytes: usize,
        now: Instant,
    ) -> std::result::Result<(), ExposeSessionError> {
        self.prune(now);
        let Some(stored) = self.sessions.get_mut(&expose_token_digest(session)) else {
            return Err(ExposeSessionError::Unauthorized);
        };
        if !expose_digest_eq(&stored.csrf_digest, &expose_token_digest(csrf)) {
            return Err(ExposeSessionError::Unauthorized);
        }
        if now.saturating_duration_since(stored.rate_window_started) >= EXPOSE_INPUT_RATE_WINDOW {
            stored.rate_window_started = now;
            stored.rate_bytes = 0;
            stored.rate_requests = 0;
        }
        if bytes > EXPOSE_INPUT_RATE_BYTES.saturating_sub(stored.rate_bytes)
            || stored.rate_requests >= EXPOSE_INPUT_RATE_REQUESTS
        {
            return Err(ExposeSessionError::RateLimited);
        }
        stored.rate_bytes += bytes;
        stored.rate_requests += 1;
        stored.last_seen = now;
        Ok(())
    }

    fn issue(
        &mut self,
        now: Instant,
    ) -> std::result::Result<ExposeIssuedSession, ExposeSessionError> {
        let session = expose_random_token().map_err(|_| ExposeSessionError::Internal)?;
        let csrf = expose_random_token().map_err(|_| ExposeSessionError::Internal)?;
        self.sessions.insert(
            expose_token_digest(&session),
            ExposeSession {
                csrf_digest: expose_token_digest(&csrf),
                expires_at: now + EXPOSE_SESSION_TTL,
                last_seen: now,
                rate_window_started: now,
                rate_bytes: 0,
                rate_requests: 0,
            },
        );
        Ok(ExposeIssuedSession { session, csrf })
    }

    fn prune(&mut self, now: Instant) {
        self.sessions.retain(|_, session| {
            now < session.expires_at
                && now.saturating_duration_since(session.last_seen) < EXPOSE_SESSION_IDLE_TIMEOUT
        });
    }
}

pub(super) fn expose_token_digest(token: &str) -> ExposeDigest {
    Sha256::digest(token.as_bytes()).into()
}

pub(super) fn expose_digest_eq(left: &ExposeDigest, right: &ExposeDigest) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (*left ^ *right)
        })
        == 0
}

pub(super) fn expose_random_token() -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).context("failed to generate expose capability")?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}
