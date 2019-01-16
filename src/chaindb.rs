//
// Copyright 2018-2019 Tamas Blummer
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
//!
//! # Blockchain DB for a node
//!

use lightchaindb::LightChainDB;
use heavychaindb::HeavyChainDB;
use error::SPVError;
use blockfilter::BlockFilter;

use bitcoin::{
    BitcoinHash,
    network::constants::Network,
    blockdata::block::Block,
    util::hash::Sha256dHash
};

use hammersbald::PRef;

pub struct ChainDB {
    light: LightChainDB,
    heavy: Option<HeavyChainDB>
}

use std::{
    path::Path
};


impl ChainDB {
    /// Create an in-memory database instance
    pub fn mem(network: Network, heavy: bool) -> Result<ChainDB, SPVError> {
        let light = LightChainDB::mem(network)?;
        if heavy {
            Ok(ChainDB { light, heavy: Some(HeavyChainDB::mem()?) })
        }
        else {
            Ok(ChainDB { light, heavy: None})
        }
    }

    /// Create or open a persistent database instance identified by the path
    pub fn new(path: &Path, network: Network, heavy: bool) -> Result<ChainDB, SPVError> {
        let light = LightChainDB::new(path, network)?;
        if heavy {
            Ok(ChainDB { light, heavy: Some(HeavyChainDB::new(path)?) })
        }
        else {
            Ok(ChainDB { light, heavy: None})
        }
    }

    pub fn init (&mut self) -> Result<(), SPVError> {
        self.light.init()
    }

    // store block if extending trunk
    pub fn extend_blocks (&mut self, block: &Block) -> Result<Option<PRef>, SPVError> {
        if let Some(heavy) = self.heavy {
            let ref block_id = block.bitcoin_hash();
            if self.light.is_on_trunk(block_id) {
                return Ok(None);
            }
            let mut blocks = heavy.blocks();
            if let Some(blocks_tip) = blocks.fetch_tip()? {
                if let Some(header) = self.light.get_header(block_id) {
                    if header.header.prev_blockhash == blocks_tip {
                        let sref = blocks.store(block)?;
                        blocks.store_tip(block_id)?;
                        return Ok(Some(sref));
                    }
                }
            }
        }
        Ok(None)
    }

    // extend UTXO store
    fn extend_utxo (&mut self, block_ref: PRef) -> Result<(), SPVError> {
        if let Some(mut heavy) = self.heavy {
            let mut utxos = heavy.utxos();
            utxos.apply_block(block_ref);
        }
        Ok(())
    }

    fn compute_filter(&mut self, block: &Block) -> Result<Option<BlockFilter>, SPVError> {
        if let Some(mut heavy) = self.heavy {
            let mut utxos = heavy.utxos().get_utxo_accessor(block)?;
            return Ok(Some(BlockFilter::compute_wallet_filter(block, utxos)?));
        }
        Ok(None)
    }

    pub fn extend_blocks_utxo_filters (&mut self, block: &Block) -> Result<(), SPVError> {
        if let Some(heavy) = self.heavy {
            if let Some(block_ref) = self.extend_blocks(block)? {
                self.extend_utxo(block_ref)?;
                if let Some(filter) = self.compute_filter(block)? {
                    self.light.add_filter(&block.bitcoin_hash(), &block.header.prev_blockhash, filter.content)?;
                }
            }
        }
        Ok(())
    }

    pub fn unwind_tip (&mut self) -> Result<Option<Sha256dHash>, SPVError> {
        if let Some(mut heavy) = self.heavy {
            heavy.unwind_tip()?;
        }
        self.light.unwind_tip()
    }
}