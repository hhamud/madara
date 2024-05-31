use std::path::PathBuf;
use std::sync::Arc;

use finality_aleph::{
    get_aleph_block_import, AllBlockMetrics, ChannelProvider, FavouriteSelectChainProvider, JustificationTranslator,
    SubstrateChainStatus, UnitCreationDelay,
};
use log::warn;
use madara_runtime::opaque::Block;
use madara_runtime::RuntimeApi;
use sc_consensus_aura::ImportQueueParams;
use sc_executor::NativeElseWasmExecutor;
use sc_service::error::Error as ServiceError;
use sc_service::{new_db_backend, Configuration, TaskManager};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sp_api::ConstructRuntimeApi;
use sp_consensus_aura::sr25519::AuthorityPair;

use crate::genesis_block::MadaraGenesisBlockBuilder;
use crate::import_queue::BlockImportPipeline;
use crate::service::{BasicImportQueue, ExecutorDispatch, FullBackend, FullClient, FullSelectChain};
use crate::starknet::{db_config_dir, MadaraBackend};

/// Contains the service components for the Aleph Consensus Mechanism
pub struct AlephBFT {
    /// AlephBFT unit creation delay (in ms)
    unit_creation_delay: u64,

    /// The addresses at which the node will be externally reachable for validator network
    /// purposes. Have to be provided for validators.
    public_validator_addresses: Option<Vec<String>>,

    /// The port on which to listen to validator network connections.
    validator_port: u16,

    /// Turn off backups, at the cost of limiting crash recoverability.
    ///
    /// If backups are turned off and the node crashes, it most likely will not be able to continue
    /// the session during which it crashed. It will join AlephBFT consensus in the next session.
    no_backup: bool,

    /// The path to save backups to.
    ///
    /// Backups created by the node are saved under this path. When restarted after a crash,
    /// the backups will be used to recover the node's state, helping prevent auto-forks. The layout
    /// of the directory is unspecified. This flag must be specified unless backups are turned off
    /// with `--no-backup`, but note that that limits crash recoverability.
    backup_path: Option<PathBuf>,

    /// The maximum number of nonfinalized blocks, after which block production should be locally
    /// stopped. DO NOT CHANGE THIS, PRODUCING MORE OR FEWER BLOCKS MIGHT BE CONSIDERED MALICIOUS
    /// BEHAVIOUR AND PUNISHED ACCORDINGLY!
    max_nonfinalized_blocks: u32,

    /// Enable database pruning. It removes older entries in the state-database. Pruning of blocks
    /// is not supported. Note that we only support pruning with ParityDB database backend.
    enable_pruning: bool,

    /// Maximum bit-rate per node in bytes per second of the alephbft validator network.
    alephbft_bit_rate_per_connection: u64,

    /// Don't spend some extra time to collect more debugging data (e.g. validator network details).
    /// By default collecting is enabled, as the impact on performance is negligible, if any.
    no_collection_of_extra_debugging_data: bool,
}

const DEFAULT_MAX_NON_FINALIZED_BLOCKS: u32 = 20;

impl AlephBFT {
    fn new() -> Self {
        Self {
            unit_creation_delay: 200,
            public_validator_addresses: None,
            validator_port: 30343,
            no_backup: true,
            backup_path: None,
            max_nonfinalized_blocks: DEFAULT_MAX_NON_FINALIZED_BLOCKS,
            enable_pruning: false,
            alephbft_bit_rate_per_connection: 64 * 1024,
            no_collection_of_extra_debugging_data: false,
        }
    }

    pub fn unit_creation_delay(&self) -> UnitCreationDelay {
        UnitCreationDelay(self.unit_creation_delay)
    }

    pub fn external_addresses(&self) -> Vec<String> {
        self.public_validator_addresses.clone().unwrap_or_default()
    }

    pub fn set_dummy_external_addresses(&mut self) {
        self.public_validator_addresses = Some(vec!["192.0.2.43:30343".to_string()])
    }

    pub fn validator_port(&self) -> u16 {
        self.validator_port
    }

    pub fn backup_path(&self) -> Option<PathBuf> {
        self.backup_path.clone()
    }

    pub fn no_backup(&self) -> bool {
        self.no_backup
    }

    pub fn max_nonfinalized_blocks(&self) -> u32 {
        if self.max_nonfinalized_blocks != DEFAULT_MAX_NON_FINALIZED_BLOCKS {
            warn!(
                "Running block production with a value of max-nonfinalized-blocks {}, which is not the default of 20. \
                 THIS MIGHT BE CONSIDERED MALICIOUS BEHAVIOUR AND RESULT IN PENALTIES!",
                self.max_nonfinalized_blocks
            );
        }
        self.max_nonfinalized_blocks
    }

    pub fn enable_pruning(&self) -> bool {
        self.enable_pruning
    }

    pub fn alephbft_bit_rate_per_connection(&self) -> u64 {
        self.alephbft_bit_rate_per_connection
    }

    pub fn no_collection_of_extra_debugging_data(&self) -> bool {
        self.no_collection_of_extra_debugging_data
    }
}

type Service = sc_service::PartialComponents<
    FullClient,
    FullBackend,
    FullSelectChain,
    BasicImportQueue,
    sc_transaction_pool::FullPool<Block, FullClient>,
    (Arc<MadaraBackend>, BlockImportPipeline, Option<Telemetry>),
>;

#[allow(clippy::type_complexity)]
pub fn new_aleph_partial(config: &Configuration) -> Result<Service, ServiceError>
where
    RuntimeApi: ConstructRuntimeApi<Block, FullClient>,
    RuntimeApi: Send + Sync + 'static,
{
    let telemetry = config
        .telemetry_endpoints
        .clone()
        .filter(|x| !x.is_empty())
        .map(|endpoints| -> Result<_, sc_telemetry::Error> {
            let worker = TelemetryWorker::new(16)?;
            let telemetry = worker.handle().new_telemetry(endpoints);
            Ok((worker, telemetry))
        })
        .transpose()?;

    let executor = sc_service::new_native_or_wasm_executor(config);

    let backend = new_db_backend(config.db_config())?;

    let genesis_block_builder = MadaraGenesisBlockBuilder::<Block, _, _>::new(
        config.chain_spec.as_storage_builder(),
        true,
        backend.clone(),
        executor.clone(),
    )
    .unwrap();

    let (client, backend, keystore_container, task_manager) = sc_service::new_full_parts_with_genesis_builder::<
        Block,
        RuntimeApi,
        _,
        MadaraGenesisBlockBuilder<Block, FullBackend, NativeElseWasmExecutor<ExecutorDispatch>>,
    >(
        config,
        telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
        executor,
        backend,
        genesis_block_builder,
    )?;

    let client = Arc::new(client);

    let telemetry = telemetry.map(|(worker, telemetry)| {
        task_manager.spawn_handle().spawn("telemetry", None, worker.run());
        telemetry
    });

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        config.transaction_pool.clone(),
        config.role.is_authority().into(),
        config.prometheus_registry(),
        task_manager.spawn_essential_handle(),
        client.clone(),
    );

    let madara_backend = Arc::new(MadaraBackend::open(&config.database, &db_config_dir(config))?);

    let justification_translator = JustificationTranslator::new(
        SubstrateChainStatus::new(backend.clone())
            .map_err(|e| ServiceError::Other(format!("failed to set up chain status: {e}")))?,
    );
    let metrics = AllBlockMetrics::new(config.prometheus_registry());
    let justification_channel_provider = ChannelProvider::new();
    let select_chain_provider = FavouriteSelectChainProvider::default();

    let aleph_block_import = get_aleph_block_import(
        client.clone(),
        justification_channel_provider.get_sender(),
        justification_translator,
        select_chain_provider.select_chain(),
        metrics.clone(),
    );

    let slot_duration = sc_consensus_aura::slot_duration(&*client)?;

    // DO NOT change Aura parameters without updating the finality-aleph sync accordingly,
    // in particular the code responsible for verifying incoming Headers, as it is supposed
    // to duplicate parts of Aura internal logic
    let import_queue = sc_consensus_aura::import_queue::<AuthorityPair, _, _, _, _, _>(ImportQueueParams {
        block_import: aleph_block_import.clone(),
        justification_import: Some(Box::new(aleph_block_import)),
        client: client.clone(),
        create_inherent_data_providers: move |_, ()| async move {
            let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

            let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                *timestamp,
                slot_duration,
            );

            Ok((slot, timestamp))
        },
        spawner: &task_manager.spawn_essential_handle(),
        registry: config.prometheus_registry(),
        check_for_equivocation: Default::default(),
        telemetry: telemetry.as_ref().map(|x| x.handle()),
        compatibility_mode: Default::default(),
    })?;

    let import_pipeline = BlockImportPipeline { block_import: Box::new(aleph_block_import), grandpa_link: None };

    Ok(sc_service::PartialComponents {
        client,
        backend,
        task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other: (madara_backend, import_pipeline, telemetry),
    })
}

pub fn new_aleph_full(config: &Configuration, aleph_config: AlephBFT) -> Result<TaskManager, ServiceError> {
    todo!()
}
