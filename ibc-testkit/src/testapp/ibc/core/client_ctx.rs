use ibc::clients::tendermint::context::ValidationContext as TmValidationContext;
use ibc::core::client::context::{ClientExecutionContext, ClientValidationContext};
use ibc::core::client::types::error::ClientError;
use ibc::core::client::types::Height;
use ibc::core::connection::types::error::ConnectionError;
use ibc::core::handler::types::error::ContextError;
use ibc::core::host::types::identifiers::{ChannelId, ClientId, PortId};
use ibc::core::host::types::path::{ClientConsensusStatePath, ClientStatePath};
use ibc::core::host::ValidationContext;
use ibc::core::primitives::Timestamp;
use ibc::primitives::prelude::*;
use ibc_proto::google::protobuf::Any;

use crate::testapp::ibc::clients::mock::client_state::{MockClientContext, MockClientState};
use crate::testapp::ibc::clients::{AnyClientState, AnyConsensusState};
use crate::testapp::ibc::core::types::MockContext;

pub type PortChannelIdMap<V> = BTreeMap<PortId, BTreeMap<ChannelId, V>>;

/// A mock of an IBC client record as it is stored in a mock context.
/// For testing ICS02 handlers mostly, cf. `MockClientContext`.
#[derive(Clone, Debug)]
pub struct MockClientRecord {
    /// The client state (representing only the latest height at the moment).
    pub client_state: Option<AnyClientState>,

    /// Mapping of heights to consensus states for this client.
    pub consensus_states: BTreeMap<Height, AnyConsensusState>,
}

impl MockClientContext for MockContext {
    fn host_timestamp(&self) -> Result<Timestamp, ContextError> {
        ValidationContext::host_timestamp(self)
    }

    fn host_height(&self) -> Result<Height, ContextError> {
        ValidationContext::host_height(self)
    }
}

impl TmValidationContext for MockContext {
    type ConversionError = &'static str;
    type AnyConsensusState = AnyConsensusState;

    fn host_timestamp(&self) -> Result<Timestamp, ContextError> {
        ValidationContext::host_timestamp(self)
    }

    fn host_height(&self) -> Result<Height, ContextError> {
        ValidationContext::host_height(self)
    }

    fn consensus_state_heights(&self, client_id: &ClientId) -> Result<Vec<Height>, ContextError> {
        let ibc_store = self.ibc_store.lock();
        let client_record =
            ibc_store
                .clients
                .get(client_id)
                .ok_or_else(|| ClientError::ClientStateNotFound {
                    client_id: client_id.clone(),
                })?;

        let heights = client_record.consensus_states.keys().cloned().collect();

        Ok(heights)
    }

    fn next_consensus_state(
        &self,
        client_id: &ClientId,
        height: &Height,
    ) -> Result<Option<Self::AnyConsensusState>, ContextError> {
        let ibc_store = self.ibc_store.lock();
        let client_record =
            ibc_store
                .clients
                .get(client_id)
                .ok_or_else(|| ClientError::ClientStateNotFound {
                    client_id: client_id.clone(),
                })?;

        // Get the consensus state heights and sort them in ascending order.
        let mut heights: Vec<Height> = client_record.consensus_states.keys().cloned().collect();
        heights.sort();

        // Search for next state.
        for h in heights {
            if h > *height {
                // unwrap should never happen, as the consensus state for h must exist
                return Ok(Some(
                    client_record
                        .consensus_states
                        .get(&h)
                        .expect("Never fails")
                        .clone(),
                ));
            }
        }
        Ok(None)
    }

    fn prev_consensus_state(
        &self,
        client_id: &ClientId,
        height: &Height,
    ) -> Result<Option<Self::AnyConsensusState>, ContextError> {
        let ibc_store = self.ibc_store.lock();
        let client_record =
            ibc_store
                .clients
                .get(client_id)
                .ok_or_else(|| ClientError::ClientStateNotFound {
                    client_id: client_id.clone(),
                })?;

        // Get the consensus state heights and sort them in descending order.
        let mut heights: Vec<Height> = client_record.consensus_states.keys().cloned().collect();
        heights.sort_by(|a, b| b.cmp(a));

        // Search for previous state.
        for h in heights {
            if h < *height {
                // unwrap should never happen, as the consensus state for h must exist
                return Ok(Some(
                    client_record
                        .consensus_states
                        .get(&h)
                        .expect("Never fails")
                        .clone(),
                ));
            }
        }
        Ok(None)
    }
}

impl ClientValidationContext for MockContext {
    type ClientStateRef = AnyClientState;
    type ConsensusStateRef = AnyConsensusState;

    fn client_state(&self, client_id: &ClientId) -> Result<Self::ClientStateRef, ContextError> {
        match self.ibc_store.lock().clients.get(client_id) {
            Some(client_record) => {
                client_record
                    .client_state
                    .clone()
                    .ok_or_else(|| ClientError::ClientStateNotFound {
                        client_id: client_id.clone(),
                    })
            }
            None => Err(ClientError::ClientStateNotFound {
                client_id: client_id.clone(),
            }),
        }
        .map_err(ContextError::ClientError)
    }

    fn consensus_state(
        &self,
        client_cons_state_path: &ClientConsensusStatePath,
    ) -> Result<AnyConsensusState, ContextError> {
        let client_id = &client_cons_state_path.client_id;
        let height = Height::new(
            client_cons_state_path.revision_number,
            client_cons_state_path.revision_height,
        )?;
        match self.ibc_store.lock().clients.get(client_id) {
            Some(client_record) => match client_record.consensus_states.get(&height) {
                Some(consensus_state) => Ok(consensus_state.clone()),
                None => Err(ClientError::ConsensusStateNotFound {
                    client_id: client_id.clone(),
                    height,
                }),
            },
            None => Err(ClientError::ConsensusStateNotFound {
                client_id: client_id.clone(),
                height,
            }),
        }
        .map_err(ContextError::ClientError)
    }

    fn self_consensus_state(&self, height: &Height) -> Result<AnyConsensusState, ContextError> {
        match self.host_block(height) {
            Some(block_ref) => Ok(block_ref.clone().into()),
            None => Err(ClientError::MissingLocalConsensusState { height: *height }),
        }
        .map_err(ConnectionError::Client)
        .map_err(ContextError::ConnectionError)
    }

    fn validate_self_client(
        &self,
        client_state_of_host_on_counterparty: Any,
    ) -> Result<(), ContextError> {
        let mock_client_state = MockClientState::try_from(client_state_of_host_on_counterparty)
            .map_err(|_| ConnectionError::InvalidClientState {
                reason: "client must be a mock client".to_string(),
            })
            .map_err(ContextError::ConnectionError)?;

        if mock_client_state.is_frozen() {
            return Err(ClientError::ClientFrozen {
                description: String::new(),
            }
            .into());
        }

        let self_chain_id = &self.host_chain_id;
        let self_revision_number = self_chain_id.revision_number();
        if self_revision_number != mock_client_state.latest_height().revision_number() {
            return Err(ContextError::ConnectionError(
                ConnectionError::InvalidClientState {
                    reason: format!(
                        "client is not in the same revision as the chain. expected: {}, got: {}",
                        self_revision_number,
                        mock_client_state.latest_height().revision_number()
                    ),
                },
            ));
        }

        let host_current_height = self.latest_height().increment();
        if mock_client_state.latest_height() >= host_current_height {
            return Err(ContextError::ConnectionError(
                ConnectionError::InvalidClientState {
                    reason: format!(
                        "client has latest height {} greater than or equal to chain height {}",
                        mock_client_state.latest_height(),
                        host_current_height
                    ),
                },
            ));
        }

        Ok(())
    }

    fn client_update_meta(
        &self,
        client_id: &ClientId,
        height: &Height,
    ) -> Result<(Timestamp, Height), ContextError> {
        let key = (client_id.clone(), *height);
        (|| {
            let ibc_store = self.ibc_store.lock();
            let time = ibc_store.client_processed_times.get(&key)?;
            let height = ibc_store.client_processed_heights.get(&key)?;
            Some((*time, *height))
        })()
        .ok_or(ClientError::UpdateMetaDataNotFound {
            client_id: key.0,
            height: key.1,
        })
        .map_err(ContextError::from)
    }
}

impl ClientExecutionContext for MockContext {
    type ClientStateMut = AnyClientState;

    fn store_client_state(
        &mut self,
        client_state_path: ClientStatePath,
        client_state: Self::ClientStateRef,
    ) -> Result<(), ContextError> {
        let mut ibc_store = self.ibc_store.lock();

        let client_id = client_state_path.0;
        let client_record = ibc_store
            .clients
            .entry(client_id)
            .or_insert(MockClientRecord {
                consensus_states: Default::default(),
                client_state: Default::default(),
            });

        client_record.client_state = Some(client_state);

        Ok(())
    }

    fn store_consensus_state(
        &mut self,
        consensus_state_path: ClientConsensusStatePath,
        consensus_state: Self::ConsensusStateRef,
    ) -> Result<(), ContextError> {
        let mut ibc_store = self.ibc_store.lock();

        let client_record = ibc_store
            .clients
            .entry(consensus_state_path.client_id)
            .or_insert(MockClientRecord {
                consensus_states: Default::default(),
                client_state: Default::default(),
            });

        let height = Height::new(
            consensus_state_path.revision_number,
            consensus_state_path.revision_height,
        )
        .expect("Never fails");
        client_record
            .consensus_states
            .insert(height, consensus_state);

        Ok(())
    }

    fn delete_consensus_state(
        &mut self,
        consensus_state_path: ClientConsensusStatePath,
    ) -> Result<(), ContextError> {
        let mut ibc_store = self.ibc_store.lock();

        let client_record = ibc_store
            .clients
            .entry(consensus_state_path.client_id)
            .or_insert(MockClientRecord {
                consensus_states: Default::default(),
                client_state: Default::default(),
            });

        let height = Height::new(
            consensus_state_path.revision_number,
            consensus_state_path.revision_height,
        )
        .expect("Never fails");

        client_record.consensus_states.remove(&height);

        Ok(())
    }

    fn delete_update_meta(
        &mut self,
        client_id: ClientId,
        height: Height,
    ) -> Result<(), ContextError> {
        let key = (client_id.clone(), height);
        let mut ibc_store = self.ibc_store.lock();
        ibc_store.client_processed_times.remove(&key);
        ibc_store.client_processed_heights.remove(&key);
        Ok(())
    }

    fn store_update_meta(
        &mut self,
        client_id: ClientId,
        height: Height,
        host_timestamp: Timestamp,
        host_height: Height,
    ) -> Result<(), ContextError> {
        let mut ibc_store = self.ibc_store.lock();
        ibc_store
            .client_processed_times
            .insert((client_id.clone(), height), host_timestamp);
        ibc_store
            .client_processed_heights
            .insert((client_id, height), host_height);
        Ok(())
    }
}
