//! Runtime switches used only by the architecture ablation runner.
//!
//! The product defaults to `full_galen`.  An experiment process can set
//! `GALEN_ARCH_VARIANT` to disable one architectural mechanism without
//! changing the user's persisted configuration.  This is intentionally
//! process-local and has no effect on normal desktop runs.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchitectureVariant {
    Stateless,
    NoDataContract,
    NoExecContract,
    NoEvidenceLink,
    GenericSameRuntime,
    FullGalen,
}

impl ArchitectureVariant {
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var("GALEN_ARCH_VARIANT")
            .unwrap_or_else(|_| "full_galen".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "stateless" => Self::Stateless,
            "no_data_contract" => Self::NoDataContract,
            "no_exec_contract" => Self::NoExecContract,
            "no_evidence_link" => Self::NoEvidenceLink,
            "generic_same_runtime" => Self::GenericSameRuntime,
            _ => Self::FullGalen,
        }
    }

    #[must_use]
    pub fn state_layer_enabled(self) -> bool {
        !matches!(self, Self::Stateless | Self::GenericSameRuntime)
    }

    #[must_use]
    pub fn data_contract_enabled(self) -> bool {
        !matches!(self, Self::NoDataContract | Self::GenericSameRuntime)
    }

    #[must_use]
    pub fn execution_contract_enabled(self) -> bool {
        !matches!(self, Self::NoExecContract | Self::GenericSameRuntime)
    }

    #[must_use]
    pub fn evidence_link_enabled(self) -> bool {
        !matches!(self, Self::NoEvidenceLink | Self::GenericSameRuntime)
    }
}

#[must_use]
pub fn current() -> ArchitectureVariant {
    ArchitectureVariant::from_env()
}

#[cfg(test)]
mod tests {
    use super::ArchitectureVariant;

    #[test]
    fn feature_matrix_is_explicit() {
        assert!(!ArchitectureVariant::Stateless.state_layer_enabled());
        assert!(!ArchitectureVariant::NoDataContract.data_contract_enabled());
        assert!(!ArchitectureVariant::NoExecContract.execution_contract_enabled());
        assert!(!ArchitectureVariant::NoEvidenceLink.evidence_link_enabled());
        assert!(ArchitectureVariant::FullGalen.state_layer_enabled());
        assert!(ArchitectureVariant::FullGalen.data_contract_enabled());
        assert!(ArchitectureVariant::FullGalen.execution_contract_enabled());
        assert!(ArchitectureVariant::FullGalen.evidence_link_enabled());
    }
}
