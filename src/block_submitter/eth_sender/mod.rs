use super::types::{ContractCall, SubmitBlockArgs};
use crate::block_submitter::Settings;
use crate::contracts;
use crate::storage::PoolType;
use crossbeam_channel::Receiver;
use ethers::abi::Abi;
use ethers::prelude::*;
use ethers::types::H256;
use fluidex_common::db::models;
use std::convert::TryFrom;

#[derive(Debug)]
pub struct EthSender {
    connpool: PoolType,
    account: Address,
    contract: Contract<Provider<Http>>,
    confirmations: usize,
}

impl EthSender {
    pub async fn from_config_with_pool(config: &Settings, connpool: PoolType) -> Result<Self, anyhow::Error> {
        let address = config.contract_address.parse::<Address>()?;
        let abi: Abi = contracts::get_abi(&config.contract_abi_file_path)?;
        let client = Provider::<Http>::try_from(config.web3_url.as_str())?; // assume wallet inside
        let account = match config.account {
            None => client.get_accounts().await?[0],
            Some(ref addr) => addr.parse::<Address>()?,
        };

        let contract = Contract::new(address, abi, client);

        Ok(Self {
            connpool,
            account,
            contract,
            confirmations: config.confirmations,
        })
    }

    pub async fn run(&self, rx: Receiver<ContractCall>) {
        for call in rx.iter() {
            log::debug!("{:?}", call);
            if let Err(e) = self.run_inner(call).await {
                log::error!("{:?}", e);
            };
        }
    }

    async fn run_inner(&self, call: ContractCall) -> Result<(), anyhow::Error> {
        match call {
            ContractCall::SubmitBlock(args) => self.submit_block(args).await,
        }
    }

    pub async fn verify_submitting(&self, args: SubmitBlockArgs) -> Result<bool, anyhow::Error> {
        let ret = self
            .contract
            .method::<_, bool>("verifySubmitting", (args.block_id, args.public_inputs, args.serialized_proof, args.public_data))?
            .call()
            .await?;
        
        Ok(ret)
    }

    pub async fn verify_block(&self, args: SubmitBlockArgs) -> Result<bool, anyhow::Error> {
        let ret = self
            .contract
            .method::<_, bool>("verifyBlock", (args.public_inputs, args.serialized_proof, args.public_data))?
            .call()
            .await?;
        
        Ok(ret)
    }

    pub async fn submit_block(&self, args: SubmitBlockArgs) -> Result<(), anyhow::Error> {
        let call = self
            .contract
            .method::<_, H256>("submitBlock", (args.block_id, args.public_inputs, args.serialized_proof, args.public_data))?
            .from(self.account);
        // ganache does not support EIP-1559
        #[cfg(feature = "ganache")]
        let call = call.legacy();
        let pending_tx = call.send().await?;
        let receipt = pending_tx.confirmations(self.confirmations).await?;
        log::info!("block {:?} confirmed. receipt: {:?}.", args.block_id, receipt);

        // https://stackoverflow.com/questions/57350082/to-convert-a-ethereum-typesh256-to-string-in-rust
        let tx_hash_str = match receipt {
            Some(receipt) => format!("{:#x}", receipt.transaction_hash),
            None => String::default(),
        };

        let stmt = format!(
            "update {} set status = $1, l1_tx_hash = $2 where block_id = $3",
            models::tablenames::L2_BLOCK
        );
        sqlx::query(&stmt)
            .bind(models::l2_block::BlockStatus::Verified)
            .bind(tx_hash_str)
            .bind(args.block_id.as_u64() as i64)
            .execute(&self.connpool)
            .await?;

        Ok(())
    }

    pub async fn submit_block_legacy(&self, args: SubmitBlockArgs) -> Result<(), anyhow::Error> {
        let call = self
            .contract
            .method::<_, H256>("submitBlockLegacy", (args.block_id, args.public_inputs, args.serialized_proof))?
            .from(self.account);
        // ganache does not support EIP-1559
        #[cfg(feature = "ganache")]
        let call = call.legacy();
        let pending_tx = call.send().await?;
        let receipt = pending_tx.confirmations(self.confirmations).await?;
        log::info!("block {:?} confirmed. receipt: {:?}.", args.block_id, receipt);

        // https://stackoverflow.com/questions/57350082/to-convert-a-ethereum-typesh256-to-string-in-rust
        let tx_hash_str = match receipt {
            Some(receipt) => format!("{:#x}", receipt.transaction_hash),
            None => String::default(),
        };

        let stmt = format!(
            "update {} set status = $1, l1_tx_hash = $2 where block_id = $3",
            models::tablenames::L2_BLOCK
        );
        sqlx::query(&stmt)
            .bind(models::l2_block::BlockStatus::Verified)
            .bind(tx_hash_str)
            .bind(args.block_id.as_u64() as i64)
            .execute(&self.connpool)
            .await?;

        Ok(())
    }
}
