use crate::model::{
    DiagnosticEvidence, DiagnosticFinding, DiagnosticTruncation, EvidenceId, FindingId,
    MAX_EVIDENCE_PER_REPORT, MAX_FINDINGS_PER_REPORT,
};
use crate::parser::{ParsedEvidence, ParsedFinding};

/// Assigns report-local identifiers and enforces retention ceilings while assembling findings.
#[derive(Debug, Default)]
pub(crate) struct Assembler {
    next_evidence: u32,
    next_finding: u32,
    findings: Vec<DiagnosticFinding>,
    evidence: Vec<DiagnosticEvidence>,
    truncation: DiagnosticTruncation,
}

impl Assembler {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn ingest_many(&mut self, parsed: &[ParsedFinding]) {
        for finding in parsed {
            self.ingest(finding);
        }
    }

    pub(crate) fn ingest(&mut self, parsed: &ParsedFinding) {
        if self.findings.len() >= MAX_FINDINGS_PER_REPORT {
            self.truncation.findings_omitted += 1;
            return;
        }

        let finding_id = FindingId::new(self.next_finding);
        self.next_finding += 1;

        let mut references = Vec::new();
        for parsed_evidence in &parsed.evidence {
            if let Some(id) = self.intern_evidence(parsed_evidence) {
                references.push(id);
            }
        }

        self.findings.push(
            DiagnosticFinding::new(finding_id, parsed.diagnostic.clone(), parsed.confidence)
                .with_evidence(references),
        );
    }

    fn intern_evidence(&mut self, parsed: &ParsedEvidence) -> Option<EvidenceId> {
        if let Some(existing) = self.evidence.iter().find(|evidence| {
            evidence.source == parsed.source
                && evidence.path == parsed.path
                && evidence.line == parsed.line
                && evidence.excerpt == parsed.excerpt
                && evidence.truncated == parsed.truncated
                && evidence.fields == parsed.fields
        }) {
            return Some(existing.id);
        }

        if self.evidence.len() >= MAX_EVIDENCE_PER_REPORT {
            self.truncation.evidence_omitted += 1;
            return None;
        }

        let id = EvidenceId::new(self.next_evidence);

        self.next_evidence += 1;
        self.evidence.push(DiagnosticEvidence {
            id,
            source: parsed.source,
            path: parsed.path.clone(),
            line: parsed.line,
            excerpt: parsed.excerpt.clone(),
            truncated: parsed.truncated,
            fields: parsed.fields.clone(),
        });

        Some(id)
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Vec<DiagnosticFinding>,
        Vec<DiagnosticEvidence>,
        DiagnosticTruncation,
    ) {
        (self.findings, self.evidence, self.truncation)
    }
}
