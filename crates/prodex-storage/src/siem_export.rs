use prodex_domain::{AuditEventId, SecretRef};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

pub const MAX_SIEM_EXPORT_BATCH_EVENTS: u16 = 256;
pub const MAX_SIEM_EXPORT_BATCH_BYTES: u32 = 1024 * 1024;
pub const MAX_SIEM_EXPORT_EVENT_BYTES: usize = 256 * 1024;

#[derive(Clone, PartialEq, Eq)]
pub struct SiemExporterCapabilities {
    pub credential_ref: SecretRef,
    pub mtls_identity_ref: Option<SecretRef>,
    pub signing_key_ref: Option<SecretRef>,
    pub max_batch_events: u16,
    pub max_batch_bytes: u32,
}

impl fmt::Debug for SiemExporterCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SiemExporterCapabilities")
            .field("credential_ref", &"<redacted>")
            .field(
                "mtls_identity_ref",
                &self.mtls_identity_ref.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "signing_key_ref",
                &self.signing_key_ref.as_ref().map(|_| "<redacted>"),
            )
            .field("max_batch_events", &self.max_batch_events)
            .field("max_batch_bytes", &self.max_batch_bytes)
            .finish()
    }
}

impl SiemExporterCapabilities {
    pub fn bounded(
        credential_ref: SecretRef,
        mtls_identity_ref: Option<SecretRef>,
        signing_key_ref: Option<SecretRef>,
        max_batch_events: u16,
        max_batch_bytes: u32,
    ) -> Result<Self, SiemExportContractError> {
        if !credential_ref.is_well_formed()
            || mtls_identity_ref
                .as_ref()
                .is_some_and(|reference| !reference.is_well_formed())
            || signing_key_ref
                .as_ref()
                .is_some_and(|reference| !reference.is_well_formed())
        {
            return Err(SiemExportContractError::InvalidSecretReference);
        }
        if max_batch_events == 0 || max_batch_events > MAX_SIEM_EXPORT_BATCH_EVENTS {
            return Err(SiemExportContractError::BatchEventLimit);
        }
        if max_batch_bytes == 0 || max_batch_bytes > MAX_SIEM_EXPORT_BATCH_BYTES {
            return Err(SiemExportContractError::BatchByteLimit);
        }
        Ok(Self {
            credential_ref,
            mtls_identity_ref,
            signing_key_ref,
            max_batch_events,
            max_batch_bytes,
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SiemExportEvent {
    pub event_id: AuditEventId,
    pub event_envelope: String,
}

impl fmt::Debug for SiemExportEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SiemExportEvent")
            .field("event_id", &"<redacted>")
            .field("event_envelope", &"<redacted>")
            .field("event_envelope_bytes", &self.event_envelope.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SiemExportBatch {
    events: Vec<SiemExportEvent>,
    total_bytes: u32,
}

impl fmt::Debug for SiemExportBatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SiemExportBatch")
            .field("event_count", &self.events.len())
            .field("total_bytes", &self.total_bytes)
            .finish()
    }
}

impl SiemExportBatch {
    pub fn bounded(
        events: Vec<SiemExportEvent>,
        capabilities: &SiemExporterCapabilities,
    ) -> Result<Self, SiemExportContractError> {
        if events.is_empty() || events.len() > usize::from(capabilities.max_batch_events) {
            return Err(SiemExportContractError::BatchEventLimit);
        }
        let mut identities = BTreeSet::new();
        let mut total_bytes = 0usize;
        for event in &events {
            if event.event_envelope.is_empty()
                || event.event_envelope.len() > MAX_SIEM_EXPORT_EVENT_BYTES
            {
                return Err(SiemExportContractError::EventByteLimit);
            }
            if !identities.insert(event.event_id) {
                return Err(SiemExportContractError::DuplicateIdempotencyKey);
            }
            total_bytes = total_bytes
                .checked_add(event.event_envelope.len())
                .ok_or(SiemExportContractError::BatchByteLimit)?;
        }
        if total_bytes > capabilities.max_batch_bytes as usize {
            return Err(SiemExportContractError::BatchByteLimit);
        }
        Ok(Self {
            events,
            total_bytes: u32::try_from(total_bytes)
                .map_err(|_| SiemExportContractError::BatchByteLimit)?,
        })
    }

    pub fn events(&self) -> &[SiemExportEvent] {
        &self.events
    }

    pub const fn total_bytes(&self) -> u32 {
        self.total_bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SiemExportReceipt {
    pub accepted_events: u16,
}

pub trait GovernanceAuditExporter {
    type Error;

    fn capabilities(&self) -> &SiemExporterCapabilities;

    fn export_batch(&mut self, batch: &SiemExportBatch) -> Result<SiemExportReceipt, Self::Error>;
}

pub trait SiemExportTransport {
    type Error;

    fn send(
        &mut self,
        capabilities: &SiemExporterCapabilities,
        batch: &SiemExportBatch,
    ) -> Result<(), Self::Error>;
}

pub struct BoundedGovernanceAuditExporter<T> {
    capabilities: SiemExporterCapabilities,
    transport: T,
}

impl<T> BoundedGovernanceAuditExporter<T> {
    pub fn new(capabilities: SiemExporterCapabilities, transport: T) -> Self {
        Self {
            capabilities,
            transport,
        }
    }
}

impl<T: SiemExportTransport> GovernanceAuditExporter for BoundedGovernanceAuditExporter<T> {
    type Error = BoundedSiemExporterError<T::Error>;

    fn capabilities(&self) -> &SiemExporterCapabilities {
        &self.capabilities
    }

    fn export_batch(&mut self, batch: &SiemExportBatch) -> Result<SiemExportReceipt, Self::Error> {
        if batch.events.len() > usize::from(self.capabilities.max_batch_events) {
            return Err(BoundedSiemExporterError::Contract(
                SiemExportContractError::BatchEventLimit,
            ));
        }
        if batch.total_bytes > self.capabilities.max_batch_bytes {
            return Err(BoundedSiemExporterError::Contract(
                SiemExportContractError::BatchByteLimit,
            ));
        }
        self.transport
            .send(&self.capabilities, batch)
            .map_err(BoundedSiemExporterError::Transport)?;
        Ok(SiemExportReceipt {
            accepted_events: batch.events.len() as u16,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoundedSiemExporterError<E> {
    Contract(SiemExportContractError),
    Transport(E),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiemExportContractError {
    InvalidSecretReference,
    BatchEventLimit,
    BatchByteLimit,
    EventByteLimit,
    DuplicateIdempotencyKey,
}

impl fmt::Display for SiemExportContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SIEM export contract is invalid")
    }
}

impl Error for SiemExportContractError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(name: &str) -> SecretRef {
        SecretRef::new("projected", name, Some("v1"))
    }

    fn capabilities() -> SiemExporterCapabilities {
        SiemExporterCapabilities::bounded(
            reference("siem-token"),
            Some(reference("siem-mtls")),
            Some(reference("siem-signing")),
            2,
            1024,
        )
        .unwrap()
    }

    struct RecordingTransport {
        calls: u16,
    }

    impl SiemExportTransport for RecordingTransport {
        type Error = ();

        fn send(
            &mut self,
            capabilities: &SiemExporterCapabilities,
            batch: &SiemExportBatch,
        ) -> Result<(), Self::Error> {
            assert!(capabilities.mtls_identity_ref.is_some());
            assert!(capabilities.signing_key_ref.is_some());
            assert_eq!(batch.events().len(), 1);
            self.calls += 1;
            Ok(())
        }
    }

    #[test]
    fn bounded_exporter_preserves_stable_idempotency_and_redacts_content() {
        let event_id = AuditEventId::new();
        let batch = SiemExportBatch::bounded(
            vec![SiemExportEvent {
                event_id,
                event_envelope: "synthetic-sensitive-audit-content".to_string(),
            }],
            &capabilities(),
        )
        .unwrap();
        assert_eq!(batch.events()[0].event_id, event_id);
        assert!(!format!("{batch:?}").contains("synthetic-sensitive"));
        assert!(!format!("{:?}", capabilities()).contains("siem-token"));

        let mut exporter =
            BoundedGovernanceAuditExporter::new(capabilities(), RecordingTransport { calls: 0 });
        assert_eq!(exporter.export_batch(&batch).unwrap().accepted_events, 1);
    }

    #[test]
    fn export_batch_rejects_duplicate_ids_and_bounded_work_overflow() {
        let id = AuditEventId::new();
        let duplicate = vec![
            SiemExportEvent {
                event_id: id,
                event_envelope: "first".to_string(),
            },
            SiemExportEvent {
                event_id: id,
                event_envelope: "second".to_string(),
            },
        ];
        assert_eq!(
            SiemExportBatch::bounded(duplicate, &capabilities()),
            Err(SiemExportContractError::DuplicateIdempotencyKey)
        );
        assert!(
            SiemExporterCapabilities::bounded(
                reference("siem-token"),
                None,
                None,
                MAX_SIEM_EXPORT_BATCH_EVENTS + 1,
                1024,
            )
            .is_err()
        );
    }

    #[test]
    fn exporter_rechecks_batch_against_its_own_capabilities() {
        let broad = SiemExporterCapabilities::bounded(reference("siem-token"), None, None, 2, 1024)
            .unwrap();
        let narrow =
            SiemExporterCapabilities::bounded(reference("siem-token"), None, None, 1, 1024)
                .unwrap();
        let batch = SiemExportBatch::bounded(
            vec![
                SiemExportEvent {
                    event_id: AuditEventId::new(),
                    event_envelope: "first".to_string(),
                },
                SiemExportEvent {
                    event_id: AuditEventId::new(),
                    event_envelope: "second".to_string(),
                },
            ],
            &broad,
        )
        .unwrap();
        let mut exporter =
            BoundedGovernanceAuditExporter::new(narrow, RecordingTransport { calls: 0 });

        assert_eq!(
            exporter.export_batch(&batch),
            Err(BoundedSiemExporterError::Contract(
                SiemExportContractError::BatchEventLimit
            ))
        );
    }
}
