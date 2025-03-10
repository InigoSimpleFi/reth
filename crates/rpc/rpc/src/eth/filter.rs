use super::cache::EthStateCache;
use crate::{
    eth::{
        error::{EthApiError, EthResult},
        logs_utils,
    },
    result::{rpc_error_with_code, ToRpcResult},
    EthSubscriptionIdProvider,
};
use alloy_primitives::B256;
use async_trait::async_trait;
use jsonrpsee::{core::RpcResult, server::IdProvider};
use reth_interfaces::RethError;
use reth_primitives::{BlockHashOrNumber, Receipt, SealedBlock, TxHash};
use reth_provider::{BlockIdReader, BlockReader, EvmEnvProvider};
use reth_rpc_api::EthFilterApiServer;
use reth_rpc_types::{Filter, FilterBlockOption, FilterChanges, FilterId, FilteredParams, Log};
use reth_tasks::TaskSpawner;
use reth_transaction_pool::TransactionPool;
use std::{
    collections::HashMap,
    iter::StepBy,
    ops::RangeInclusive,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc::Receiver, Mutex},
    time::MissedTickBehavior,
};
use tracing::trace;

/// The maximum number of headers we read at once when handling a range filter.
const MAX_HEADERS_RANGE: u64 = 1_000; // with ~530bytes per header this is ~500kb

/// `Eth` filter RPC implementation.
pub struct EthFilter<Provider, Pool> {
    /// All nested fields bundled together.
    inner: Arc<EthFilterInner<Provider, Pool>>,
}

impl<Provider, Pool> EthFilter<Provider, Pool>
where
    Provider: Send + Sync + 'static,
    Pool: Send + Sync + 'static,
{
    /// Creates a new, shareable instance.
    ///
    /// This uses the given pool to get notified about new transactions, the provider to interact
    /// with the blockchain, the cache to fetch cacheable data, like the logs and the
    /// max_logs_per_response to limit the amount of logs returned in a single response
    /// `eth_getLogs`
    ///
    /// This also spawns a task that periodically clears stale filters.
    pub fn new(
        provider: Provider,
        pool: Pool,
        eth_cache: EthStateCache,
        max_logs_per_response: usize,
        task_spawner: Box<dyn TaskSpawner>,
        stale_filter_ttl: Duration,
    ) -> Self {
        let inner = EthFilterInner {
            provider,
            active_filters: Default::default(),
            pool,
            id_provider: Arc::new(EthSubscriptionIdProvider::default()),
            max_logs_per_response,
            eth_cache,
            max_headers_range: MAX_HEADERS_RANGE,
            task_spawner,
            stale_filter_ttl,
        };

        let eth_filter = Self { inner: Arc::new(inner) };

        let this = eth_filter.clone();
        eth_filter.inner.task_spawner.clone().spawn_critical(
            "eth-filters_stale-filters-clean",
            Box::pin(async move {
                this.watch_and_clear_stale_filters().await;
            }),
        );

        eth_filter
    }

    /// Returns all currently active filters
    pub fn active_filters(&self) -> &ActiveFilters {
        &self.inner.active_filters
    }

    /// Endless future that [Self::clear_stale_filters] every `stale_filter_ttl` interval.
    async fn watch_and_clear_stale_filters(&self) {
        let mut interval = tokio::time::interval(self.inner.stale_filter_ttl);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            self.clear_stale_filters(Instant::now()).await;
        }
    }

    /// Clears all filters that have not been polled for longer than the configured
    /// `stale_filter_ttl` at the given instant.
    pub async fn clear_stale_filters(&self, now: Instant) {
        trace!(target: "rpc::eth", "clear stale filters");
        self.active_filters().inner.lock().await.retain(|id, filter| {
            let is_valid = (now - filter.last_poll_timestamp) < self.inner.stale_filter_ttl;

            if !is_valid {
                trace!(target: "rpc::eth", "evict filter with id: {:?}", id);
            }

            is_valid
        })
    }
}

impl<Provider, Pool> EthFilter<Provider, Pool>
where
    Provider: BlockReader + BlockIdReader + EvmEnvProvider + 'static,
    Pool: TransactionPool + 'static,
{
    /// Returns all the filter changes for the given id, if any
    pub async fn filter_changes(&self, id: FilterId) -> Result<FilterChanges, FilterError> {
        let info = self.inner.provider.chain_info()?;
        let best_number = info.best_number;

        // start_block is the block from which we should start fetching changes, the next block from
        // the last time changes were polled, in other words the best block at last poll + 1
        let (start_block, kind) = {
            let mut filters = self.inner.active_filters.inner.lock().await;
            let filter = filters.get_mut(&id).ok_or(FilterError::FilterNotFound(id))?;

            if filter.block > best_number {
                // no new blocks since the last poll
                return Ok(FilterChanges::Empty)
            }

            // update filter
            // we fetch all changes from [filter.block..best_block], so we advance the filter's
            // block to `best_block +1`, the next from which we should start fetching changes again
            let mut block = best_number + 1;
            std::mem::swap(&mut filter.block, &mut block);
            filter.last_poll_timestamp = Instant::now();

            (block, filter.kind.clone())
        };

        match kind {
            FilterKind::PendingTransaction(receiver) => {
                let pending_txs = receiver.drain().await;
                Ok(FilterChanges::Hashes(pending_txs))
            }
            FilterKind::Block => {
                // Note: we need to fetch the block hashes from inclusive range
                // [start_block..best_block]
                let end_block = best_number + 1;
                let block_hashes = self
                    .inner
                    .provider
                    .canonical_hashes_range(start_block, end_block)
                    .map_err(|_| EthApiError::UnknownBlockNumber)?;
                Ok(FilterChanges::Hashes(block_hashes))
            }
            FilterKind::Log(filter) => {
                let (from_block_number, to_block_number) = match filter.block_option {
                    FilterBlockOption::Range { from_block, to_block } => {
                        let from = from_block
                            .map(|num| self.inner.provider.convert_block_number(num))
                            .transpose()?
                            .flatten();
                        let to = to_block
                            .map(|num| self.inner.provider.convert_block_number(num))
                            .transpose()?
                            .flatten();
                        logs_utils::get_filter_block_range(from, to, start_block, info)
                    }
                    FilterBlockOption::AtBlockHash(_) => {
                        // blockHash is equivalent to fromBlock = toBlock = the block number with
                        // hash blockHash
                        // get_logs_in_block_range is inclusive
                        (start_block, best_number)
                    }
                };

                let logs = self
                    .inner
                    .get_logs_in_block_range(&filter, from_block_number, to_block_number)
                    .await?;
                Ok(FilterChanges::Logs(logs))
            }
        }
    }

    /// Returns an array of all logs matching filter with given id.
    ///
    /// Returns an error if no matching log filter exists.
    ///
    /// Handler for `eth_getFilterLogs`
    pub async fn filter_logs(&self, id: FilterId) -> Result<FilterChanges, FilterError> {
        let filter = {
            let filters = self.inner.active_filters.inner.lock().await;
            if let FilterKind::Log(ref filter) =
                filters.get(&id).ok_or_else(|| FilterError::FilterNotFound(id.clone()))?.kind
            {
                *filter.clone()
            } else {
                // Not a log filter
                return Err(FilterError::FilterNotFound(id))
            }
        };

        let logs = self.inner.logs_for_filter(filter).await?;
        Ok(FilterChanges::Logs(logs))
    }
}

#[async_trait]
impl<Provider, Pool> EthFilterApiServer for EthFilter<Provider, Pool>
where
    Provider: BlockReader + BlockIdReader + EvmEnvProvider + 'static,
    Pool: TransactionPool + 'static,
{
    /// Handler for `eth_newFilter`
    async fn new_filter(&self, filter: Filter) -> RpcResult<FilterId> {
        trace!(target: "rpc::eth", "Serving eth_newFilter");
        self.inner.install_filter(FilterKind::Log(Box::new(filter))).await
    }

    /// Handler for `eth_newBlockFilter`
    async fn new_block_filter(&self) -> RpcResult<FilterId> {
        trace!(target: "rpc::eth", "Serving eth_newBlockFilter");
        self.inner.install_filter(FilterKind::Block).await
    }

    /// Handler for `eth_newPendingTransactionFilter`
    async fn new_pending_transaction_filter(&self) -> RpcResult<FilterId> {
        trace!(target: "rpc::eth", "Serving eth_newPendingTransactionFilter");
        let receiver = self.inner.pool.pending_transactions_listener();

        let pending_txs_receiver = PendingTransactionsReceiver::new(receiver);

        self.inner.install_filter(FilterKind::PendingTransaction(pending_txs_receiver)).await
    }

    /// Handler for `eth_getFilterChanges`
    async fn filter_changes(&self, id: FilterId) -> RpcResult<FilterChanges> {
        trace!(target: "rpc::eth", "Serving eth_getFilterChanges");
        Ok(EthFilter::filter_changes(self, id).await?)
    }

    /// Returns an array of all logs matching filter with given id.
    ///
    /// Returns an error if no matching log filter exists.
    ///
    /// Handler for `eth_getFilterLogs`
    async fn filter_logs(&self, id: FilterId) -> RpcResult<FilterChanges> {
        trace!(target: "rpc::eth", "Serving eth_getFilterLogs");
        Ok(EthFilter::filter_logs(self, id).await?)
    }

    /// Handler for `eth_uninstallFilter`
    async fn uninstall_filter(&self, id: FilterId) -> RpcResult<bool> {
        trace!(target: "rpc::eth", "Serving eth_uninstallFilter");
        let mut filters = self.inner.active_filters.inner.lock().await;
        if filters.remove(&id).is_some() {
            trace!(target: "rpc::eth::filter", ?id, "uninstalled filter");
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Returns logs matching given filter object.
    ///
    /// Handler for `eth_getLogs`
    async fn logs(&self, filter: Filter) -> RpcResult<Vec<Log>> {
        trace!(target: "rpc::eth", "Serving eth_getLogs");
        Ok(self.inner.logs_for_filter(filter).await?)
    }
}

impl<Provider, Pool> std::fmt::Debug for EthFilter<Provider, Pool> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EthFilter").finish_non_exhaustive()
    }
}

impl<Provider, Pool> Clone for EthFilter<Provider, Pool> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

/// Container type `EthFilter`
#[derive(Debug)]
struct EthFilterInner<Provider, Pool> {
    /// The transaction pool.
    pool: Pool,
    /// The provider that can interact with the chain.
    provider: Provider,
    /// All currently installed filters.
    active_filters: ActiveFilters,
    /// Provides ids to identify filters
    id_provider: Arc<dyn IdProvider>,
    /// Maximum number of logs that can be returned in a response
    max_logs_per_response: usize,
    /// The async cache frontend for eth related data
    eth_cache: EthStateCache,
    /// maximum number of headers to read at once for range filter
    max_headers_range: u64,
    /// The type that can spawn tasks.
    task_spawner: Box<dyn TaskSpawner>,
    /// Duration since the last filter poll, after which the filter is considered stale
    stale_filter_ttl: Duration,
}

impl<Provider, Pool> EthFilterInner<Provider, Pool>
where
    Provider: BlockReader + BlockIdReader + EvmEnvProvider + 'static,
    Pool: TransactionPool + 'static,
{
    /// Returns logs matching given filter object.
    async fn logs_for_filter(&self, filter: Filter) -> Result<Vec<Log>, FilterError> {
        match filter.block_option {
            FilterBlockOption::AtBlockHash(block_hash) => {
                let mut all_logs = Vec::new();
                // all matching logs in the block, if it exists
                if let Some((block, receipts)) =
                    self.eth_cache.get_block_and_receipts(block_hash).await?
                {
                    let filter = FilteredParams::new(Some(filter));
                    logs_utils::append_matching_block_logs(
                        &mut all_logs,
                        &filter,
                        (block_hash, block.number).into(),
                        block.body.into_iter().map(|tx| tx.hash()).zip(receipts),
                        false,
                    );
                }
                Ok(all_logs)
            }
            FilterBlockOption::Range { from_block, to_block } => {
                // compute the range
                let info = self.provider.chain_info()?;

                // we start at the most recent block if unset in filter
                let start_block = info.best_number;
                let from = from_block
                    .map(|num| self.provider.convert_block_number(num))
                    .transpose()?
                    .flatten();
                let to = to_block
                    .map(|num| self.provider.convert_block_number(num))
                    .transpose()?
                    .flatten();
                let (from_block_number, to_block_number) =
                    logs_utils::get_filter_block_range(from, to, start_block, info);
                self.get_logs_in_block_range(&filter, from_block_number, to_block_number).await
            }
        }
    }

    /// Installs a new filter and returns the new identifier.
    async fn install_filter(&self, kind: FilterKind) -> RpcResult<FilterId> {
        let last_poll_block_number = self.provider.best_block_number().to_rpc_result()?;
        let id = FilterId::from(self.id_provider.next_id());
        let mut filters = self.active_filters.inner.lock().await;
        filters.insert(
            id.clone(),
            ActiveFilter {
                block: last_poll_block_number,
                last_poll_timestamp: Instant::now(),
                kind,
            },
        );
        Ok(id)
    }

    /// Fetches both receipts and block for the given block number.
    async fn block_and_receipts_by_number(
        &self,
        hash_or_number: BlockHashOrNumber,
    ) -> EthResult<Option<(SealedBlock, Vec<Receipt>)>> {
        let block_hash = match self.provider.convert_block_hash(hash_or_number)? {
            Some(hash) => hash,
            None => return Ok(None),
        };

        Ok(self.eth_cache.get_block_and_receipts(block_hash).await?)
    }

    /// Returns all logs in the given _inclusive_ range that match the filter
    ///
    /// Returns an error if:
    ///  - underlying database error
    ///  - amount of matches exceeds configured limit
    async fn get_logs_in_block_range(
        &self,
        filter: &Filter,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<Log>, FilterError> {
        trace!(target: "rpc::eth::filter", from=from_block, to=to_block, ?filter, "finding logs in range");

        let mut all_logs = Vec::new();
        let filter_params = FilteredParams::new(Some(filter.clone()));

        // derive bloom filters from filter input
        let address_filter = FilteredParams::address_filter(&filter.address);
        let topics_filter = FilteredParams::topics_filter(&filter.topics);

        let is_multi_block_range = from_block != to_block;

        // loop over the range of new blocks and check logs if the filter matches the log's bloom
        // filter
        for (from, to) in
            BlockRangeInclusiveIter::new(from_block..=to_block, self.max_headers_range)
        {
            let headers = self.provider.headers_range(from..=to)?;

            for (idx, header) in headers.iter().enumerate() {
                // these are consecutive headers, so we can use the parent hash of the next block to
                // get the current header's hash
                let num_hash: BlockHashOrNumber = headers
                    .get(idx + 1)
                    .map(|h| h.parent_hash.into())
                    .unwrap_or_else(|| header.number.into());

                // only if filter matches
                if FilteredParams::matches_address(header.logs_bloom, &address_filter) &&
                    FilteredParams::matches_topics(header.logs_bloom, &topics_filter)
                {
                    if let Some((block, receipts)) =
                        self.block_and_receipts_by_number(num_hash).await?
                    {
                        let block_hash = block.hash;

                        logs_utils::append_matching_block_logs(
                            &mut all_logs,
                            &filter_params,
                            (block.number, block_hash).into(),
                            block.body.into_iter().map(|tx| tx.hash()).zip(receipts),
                            false,
                        );

                        // size check but only if range is multiple blocks, so we always return all
                        // logs of a single block
                        if is_multi_block_range && all_logs.len() > self.max_logs_per_response {
                            return Err(FilterError::QueryExceedsMaxResults(
                                self.max_logs_per_response,
                            ))
                        }
                    }
                }
            }
        }

        Ok(all_logs)
    }
}

/// All active filters
#[derive(Debug, Clone, Default)]
pub struct ActiveFilters {
    inner: Arc<Mutex<HashMap<FilterId, ActiveFilter>>>,
}

/// An installed filter
#[derive(Debug)]
struct ActiveFilter {
    /// At which block the filter was polled last.
    block: u64,
    /// Last time this filter was polled.
    last_poll_timestamp: Instant,
    /// What kind of filter it is.
    kind: FilterKind,
}

/// A receiver for pending transactions that returns all new transactions since the last poll.
#[derive(Debug, Clone)]
struct PendingTransactionsReceiver {
    txs_receiver: Arc<Mutex<Receiver<TxHash>>>,
}

impl PendingTransactionsReceiver {
    fn new(receiver: Receiver<TxHash>) -> Self {
        PendingTransactionsReceiver { txs_receiver: Arc::new(Mutex::new(receiver)) }
    }

    /// Returns all new pending transactions received since the last poll.
    async fn drain(&self) -> Vec<B256> {
        let mut pending_txs = Vec::new();
        let mut prepared_stream = self.txs_receiver.lock().await;

        while let Ok(tx_hash) = prepared_stream.try_recv() {
            pending_txs.push(tx_hash);
        }
        pending_txs
    }
}

#[derive(Clone, Debug)]
enum FilterKind {
    Log(Box<Filter>),
    Block,
    PendingTransaction(PendingTransactionsReceiver),
}

/// Errors that can occur in the handler implementation
#[derive(Debug, thiserror::Error)]
pub enum FilterError {
    #[error("filter not found")]
    FilterNotFound(FilterId),
    #[error("query exceeds max results {0}")]
    QueryExceedsMaxResults(usize),
    #[error(transparent)]
    EthAPIError(#[from] EthApiError),
    /// Error thrown when a spawned task failed to deliver a response.
    #[error("internal filter error")]
    InternalError,
}

// convert the error
impl From<FilterError> for jsonrpsee::types::error::ErrorObject<'static> {
    fn from(err: FilterError) -> Self {
        match err {
            FilterError::FilterNotFound(_) => rpc_error_with_code(
                jsonrpsee::types::error::INVALID_PARAMS_CODE,
                "filter not found",
            ),
            err @ FilterError::InternalError => {
                rpc_error_with_code(jsonrpsee::types::error::INTERNAL_ERROR_CODE, err.to_string())
            }
            FilterError::EthAPIError(err) => err.into(),
            err @ FilterError::QueryExceedsMaxResults(_) => {
                rpc_error_with_code(jsonrpsee::types::error::INVALID_PARAMS_CODE, err.to_string())
            }
        }
    }
}

impl From<RethError> for FilterError {
    fn from(err: RethError) -> Self {
        FilterError::EthAPIError(err.into())
    }
}

/// An iterator that yields _inclusive_ block ranges of a given step size
#[derive(Debug)]
struct BlockRangeInclusiveIter {
    iter: StepBy<RangeInclusive<u64>>,
    step: u64,
    end: u64,
}

impl BlockRangeInclusiveIter {
    fn new(range: RangeInclusive<u64>, step: u64) -> Self {
        Self { end: *range.end(), iter: range.step_by(step as usize + 1), step }
    }
}

impl Iterator for BlockRangeInclusiveIter {
    type Item = (u64, u64);

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.iter.next()?;
        let end = (start + self.step).min(self.end);
        if start > end {
            return None
        }
        Some((start, end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{thread_rng, Rng};

    #[test]
    fn test_block_range_iter() {
        for _ in 0..100 {
            let mut rng = thread_rng();
            let start = rng.gen::<u32>() as u64;
            let end = start.saturating_add(rng.gen::<u32>() as u64);
            let step = rng.gen::<u16>() as u64;
            let range = start..=end;
            let mut iter = BlockRangeInclusiveIter::new(range.clone(), step);
            let (from, mut end) = iter.next().unwrap();
            assert_eq!(from, start);
            assert_eq!(end, (from + step).min(*range.end()));

            for (next_from, next_end) in iter {
                // ensure range starts with previous end + 1
                assert_eq!(next_from, end + 1);
                end = next_end;
            }

            assert_eq!(end, *range.end());
        }
    }
}
