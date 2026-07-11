use anyhow::Result;
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimePolicyValidationSection {
    Version,
    ServiceMode,
    Runtime,
    Secrets,
    RuntimeProxy,
    Gateway,
    GatewayState,
    GatewayAdminTokens,
    GatewaySso,
    GatewayRouting,
    GatewayVirtualKeys,
    GatewayObservability,
    GatewayGuardrails,
}

impl fmt::Display for RuntimePolicyValidationSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Version => "version",
            Self::ServiceMode => "service_mode",
            Self::Runtime => "runtime",
            Self::Secrets => "secrets",
            Self::RuntimeProxy => "runtime_proxy",
            Self::Gateway => "gateway",
            Self::GatewayState => "gateway.state",
            Self::GatewayAdminTokens => "gateway.admin_tokens",
            Self::GatewaySso => "gateway.sso",
            Self::GatewayRouting => "gateway.routing",
            Self::GatewayVirtualKeys => "gateway.virtual_keys",
            Self::GatewayObservability => "gateway.observability",
            Self::GatewayGuardrails => "gateway.guardrails",
        };
        f.write_str(name)
    }
}

pub struct RuntimePolicyValidationIssue {
    section: RuntimePolicyValidationSection,
    message: String,
    source: anyhow::Error,
}

impl RuntimePolicyValidationIssue {
    pub fn section(&self) -> RuntimePolicyValidationSection {
        self.section
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Debug for RuntimePolicyValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimePolicyValidationIssue")
            .field("section", &self.section)
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for RuntimePolicyValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for RuntimePolicyValidationIssue {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

#[derive(Default)]
pub struct RuntimePolicyValidationErrors {
    issues: Vec<RuntimePolicyValidationIssue>,
}

impl RuntimePolicyValidationErrors {
    pub fn issues(&self) -> &[RuntimePolicyValidationIssue] {
        &self.issues
    }

    pub(crate) fn capture(&mut self, section: RuntimePolicyValidationSection, result: Result<()>) {
        if let Err(source) = result {
            self.issues.push(RuntimePolicyValidationIssue {
                section,
                message: source.to_string(),
                source,
            });
        }
    }

    pub(crate) fn finish(self) -> Result<()> {
        if self.issues.is_empty() {
            Ok(())
        } else {
            Err(anyhow::Error::new(self))
        }
    }
}

impl fmt::Debug for RuntimePolicyValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimePolicyValidationErrors")
            .field("issues", &self.issues)
            .finish()
    }
}

impl fmt::Display for RuntimePolicyValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let [issue] = self.issues.as_slice() {
            return issue.fmt(f);
        }
        write!(
            f,
            "runtime policy validation failed with {} errors",
            self.issues.len()
        )?;
        for issue in &self.issues {
            write!(f, "; [{}] {issue}", issue.section)?;
        }
        Ok(())
    }
}

impl Error for RuntimePolicyValidationErrors {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.issues.first().map(|issue| issue as &dyn Error)
    }
}
