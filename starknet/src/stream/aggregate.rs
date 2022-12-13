//! Aggregate data for one block.

use std::sync::Arc;

use crate::{
    core::{pb::starknet::v1alpha2, GlobalBlockId},
    db::StorageReader,
};

pub trait BlockDataAggregator {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Returns a `Block` with data for the given block.
    ///
    /// If there is no data for the given block, it returns `None`.
    fn aggregate_for_block(
        &self,
        block_id: &GlobalBlockId,
    ) -> Result<Option<v1alpha2::Block>, Self::Error>;

    fn next_block(&self, block_id: &GlobalBlockId) -> Result<Option<GlobalBlockId>, Self::Error>;

    fn aggregate_batch(
        &self,
        starting_block: &GlobalBlockId,
        size: usize,
        status: v1alpha2::BlockStatus,
    ) -> Result<(Vec<v1alpha2::Block>, GlobalBlockId), Self::Error> {
        let mut blocks = Vec::with_capacity(size);
        let mut current_block = *starting_block;
        for _ in 0..size {
            match self.aggregate_for_block(&current_block)? {
                None => return Ok((blocks, current_block)),
                Some(block) => {
                    if block.status() != status {
                        break;
                    }
                    blocks.push(block);
                }
            };
            match self.next_block(&current_block)? {
                None => return Ok((blocks, current_block)),
                Some(block_id) => {
                    current_block = block_id;
                }
            }
        }
        return Ok((blocks, current_block));
    }
}

pub struct DatabaseBlockDataAggregator<R: StorageReader> {
    storage: Arc<R>,
    filter: v1alpha2::Filter,
}

impl<R> DatabaseBlockDataAggregator<R>
where
    R: StorageReader,
{
    pub fn new(storage: Arc<R>, filter: v1alpha2::Filter) -> Self {
        DatabaseBlockDataAggregator { storage, filter }
    }

    fn status(&self, block_id: &GlobalBlockId) -> Result<v1alpha2::BlockStatus, R::Error> {
        let status = self
            .storage
            .read_status(&block_id)?
            .unwrap_or(v1alpha2::BlockStatus::Unspecified);
        Ok(status)
    }

    fn header(&self, block_id: &GlobalBlockId) -> Result<Option<v1alpha2::BlockHeader>, R::Error> {
        if self.filter.header.is_some() {
            self.storage.read_header(&block_id)
        } else {
            Ok(None)
        }
    }

    fn transactions(
        &self,
        block_id: &GlobalBlockId,
    ) -> Result<Vec<v1alpha2::Transaction>, R::Error> {
        if self.filter.transactions.len() == 0 {
            return Ok(Vec::default());
        }

        let transactions = self
            .storage
            .read_body(block_id)?
            .into_iter()
            .filter(|tx| self.filter_transaction(tx))
            .collect();
        Ok(transactions)
    }

    fn receipts(
        &self,
        block_id: &GlobalBlockId,
    ) -> Result<Vec<v1alpha2::TransactionReceipt>, R::Error> {
        /*
        if self.filter.receipts.len() == 0 {
            return Ok(Vec::default());
        }
        */
        Ok(Vec::default())
    }

    fn events(&self, block_id: &GlobalBlockId) -> Result<Vec<v1alpha2::Event>, R::Error> {
        if self.filter.events.len() == 0 {
            return Ok(Vec::default());
        }

        let receipts = self.storage.read_receipts(block_id)?;
        let mut events = Vec::default();
        for receipt in receipts {
            for event in receipt.events {
                for filter in &self.filter.events {
                    if filter.matches(&event) {
                        events.push(event);
                        break;
                    }
                }
            }
        }
        Ok(events)
    }

    fn state_update(
        &self,
        block_id: &GlobalBlockId,
    ) -> Result<Option<v1alpha2::StateUpdate>, R::Error> {
        // TODO: change state update flag to be a filter
        /*
        if self.filter.include_state_update {
            self.storage.read_state_update(&block_id)
        } else {
            Ok(None)
        }
        */
        Ok(None)
    }

    fn filter_transaction(&self, tx: &v1alpha2::Transaction) -> bool {
        for filter in &self.filter.transactions {
            if filter.matches(tx) {
                return true;
            }
        }
        false
    }
}

impl<R> BlockDataAggregator for DatabaseBlockDataAggregator<R>
where
    R: StorageReader,
{
    type Error = R::Error;

    #[tracing::instrument(level = "trace", skip(self))]
    fn aggregate_for_block(
        &self,
        block_id: &GlobalBlockId,
    ) -> Result<Option<v1alpha2::Block>, Self::Error> {
        let status = self.status(block_id)?;
        let header = self.header(block_id)?;
        let transactions = self.transactions(block_id)?;
        let receipts = self.receipts(block_id)?;
        let events = self.events(block_id)?;
        let state_update = self.state_update(block_id)?;

        let block = v1alpha2::Block {
            status: status as i32,
            header,
            state_update,
            transactions,
            receipts,
            events,
        };

        Ok(Some(block))
    }

    #[tracing::instrument(level = "trace", skip(self))]
    fn next_block(&self, block_id: &GlobalBlockId) -> Result<Option<GlobalBlockId>, Self::Error> {
        self.storage.canonical_block_id(block_id.number() + 1)
    }
}
