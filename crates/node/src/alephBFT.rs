use std::path::PathBuf;
use std::sync::Arc;

use finality_aleph::{
    run_validator_node, AlephBlockImport, AlephConfig, AllBlockMetrics, BlockImporter, ChannelProvider, Justification,
    JustificationTranslator, MillisecsPerBlock, Protocol, ProtocolNaming, RateLimiterConfig, RedirectingBlockImport,
    SessionPeriod, SubstrateChainStatus, SubstrateNetwork, SyncOracle, TracingBlockImport, ValidatorAddressCache,
};
use sc_service::error::Error as ServiceError;
use madara_runtime::Block;
use sc_consensus_aura::{ImportQueueParams, SlotProportion, StartAuraParams};
use sc_service::{Configuration, TaskManager};
use sc_telemetry::Telemetry;

use crate::import_queue::BlockImportPipeline;
use crate::service::{BasicImportQueue, FullClient, FullSelectChain, FullBackend};
use crate::starknet::MadaraBackend;


/// Contains the service components for the Aleph Consensus Mechanism
pub struct AlephBFT {

    ///AlephBFT unit creation delay (in ms)
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

impl AlephBFT {
    fn new() -> Self {
        Self {
            unit_creation_delay: 200,
            public_validator_addresses: None,
            validator_port: 30343,
            no_backup: true,
            backup_path: None,
            max_nonfinalized_blocks: 20,
            enable_pruning: false,
            alephbft_bit_rate_per_connection: 64 * 1024,
            no_collection_of_extra_debugging_data: false,
        }
    }
}



type FullPool = sc_transaction_pool::FullPool<Block, FullClient>;
type Service = sc_service::PartialComponents<
    FullClient,
    FullBackend,
    FullSelectChain,
    BasicImportQueue,
    FullPool,
    (Arc<MadaraBackend>, BlockImportPipeline, Option<Telemetry>),
>;


pub fn new_aleph_partial(config: &Configuration) -> Result<Service, ServiceError> {
    todo!()
}

pub fn new_aleph_full(config: &Configuration) -> Result<TaskManager, ServiceError> {
    todo!()
}
