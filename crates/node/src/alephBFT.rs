use finality_aleph::{
    run_validator_node, AlephBlockImport, AlephConfig, AllBlockMetrics, BlockImporter, Justification,
    JustificationTranslator, MillisecsPerBlock, Protocol, ProtocolNaming, RateLimiterConfig, RedirectingBlockImport,
    SessionPeriod, SubstrateChainStatus, SubstrateNetwork, SyncOracle, TracingBlockImport, ValidatorAddressCache,
};

/// Contains the service components for the Aleph Consensus Mechanism
pub struct alephBFT {}

/// Build a block import queue & pipeline for default sealing.
///
/// If Starknet block import (Sierra class verification) is enabled for prod:
///     Queue (external blocks): AuraVerifier -> StarknetBlockImport -> AlephBlockImport -> Client
///     Pipeline (authored blocks): AlephBlockImport -> Client
///
/// If Starknet block import is enabled for testing:
///     Pipeline (authored blocks): StarknetBlockImport -> AlephBlockImport -> Client
///
/// Otherwise:
///     Queue (external blocks): AuraVerifier -> AlephBlockImport -> Client
///     Pipeline (authored blocks): AlephBlockImport -> Client
#[allow(unused_variables)]
pub fn build_aura_queue_aleph_pipeline(
    client: Arc<FullClient>,
    config: &Configuration,
    task_manager: &TaskManager,
    telemetry: &Option<Telemetry>,
    select_chain: FullSelectChain,
    madara_backend: Arc<MadaraBackend>,
) -> (BasicImportQueue, BlockImportPipeline) {
    let (block_import, link) = sc_consensus_grandpa::block_import(
        client.clone(),
        GRANDPA_JUSTIFICATION_PERIOD,
        &client as &Arc<_>,
        select_chain,
        telemetry.as_ref().map(|x| x.handle()),
    )?;

    let import_queue = build_aura_import_queue(
        client.clone(),
        config,
        task_manager,
        telemetry,
        Box::new(block_import.clone()),
        Some(Box::new(block_import.clone())),
    )?;

    // We do not run Sierra class verification for own blocks, unless it's for testing purposes.
    // Here we return GrandpaBlockImport which is wrapped by the outer StarknetBlockImport.
    #[cfg(all(feature = "sn-block-import", not(feature = "sn-block-import-testing")))]
    let block_import = block_import.unwrap();

    let import_pipeline = BlockImportPipeline { block_import: Box::new(block_import), grandpa_link: Some(link) };

    (import_queue, import_pipeline)
}
