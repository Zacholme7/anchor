use super::{
    CommitteeInstanceId, Completed, QbftDecidable, QbftError, QbftManager, WrappedQbftMessage,
};
use processor::Senders;
use qbft::Message;
use slot_clock::{SlotClock, SystemTimeSlotClock};
use ssv_types::consensus::{BeaconVote, QbftMessage};
use ssv_types::message::SignedSSVMessage;
use ssv_types::{Cluster, ClusterId, OperatorId};
use ssz::Decode;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use task_executor::{ShutdownReason, TaskExecutor};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use types::{Hash256, Slot};

// The only allowed qbft committee sizes
#[derive(Debug, Copy, Clone)]
pub enum CommitteeSize {
    Four = 4,
    Seven = 7,
    Ten = 10,
    Thirteen = 13,
}

impl CommitteeSize {
    // The number of fault nodes that the committee can tolerate
    fn get_f(&self) -> u64 {
        match self {
            CommitteeSize::Four => 1,
            CommitteeSize::Seven => 2,
            CommitteeSize::Ten => 3,
            CommitteeSize::Thirteen => 4,
        }
    }
}

// Represents a running qbft instance
struct RunningInstance<T, D>
where
    T: SlotClock + 'static,
    D: QbftDecidable<T>,
{
    // The qbft manager for this instance
    manager: Arc<QbftManager<T>>,
    completion_rx: oneshot::Receiver<Completed<D>>,
}

/// Configuration for an individual qbft test instance
#[derive(Debug)]
pub struct TestInstanceConfig<T, D>
where
    T: SlotClock + 'static,
    D: QbftDecidable<T>,
{
    pub id: D::Id,
    pub initial_data: D,
}

// Builder for constructing customized test instances
pub struct TestInstanceConfigBuilder<T, D>
where
    T: SlotClock + 'static,
    D: QbftDecidable<T>,
{
    id: Option<D::Id>,
    initial_data: Option<D>,
    _tmp_t: PhantomData<T>,
}

impl<T, D> TestInstanceConfigBuilder<T, D>
where
    T: SlotClock + 'static,
    D: QbftDecidable<T>,
{
    pub fn new() -> Self {
        TestInstanceConfigBuilder {
            id: None,
            initial_data: None,
            _tmp_t: PhantomData,
        }
    }

    pub fn with_data(mut self, data: D) -> Self {
        self.initial_data = Some(data);
        self
    }

    pub fn with_id(mut self, id: D::Id) -> Self {
        self.id = Some(id);
        self
    }

    pub fn build(self) -> TestInstanceConfig<T, D> {
        TestInstanceConfig {
            id: self.id.unwrap(),
            initial_data: self.initial_data.unwrap(),
        }
    }
}

/// The main test coordinator that manages multiple QBFT instances
pub struct QbftTester<T, D>
where
    T: SlotClock + 'static,
    D: QbftDecidable<T>,
{
    // Senders to the processor
    senders: Senders,
    // Track all of the running instances
    instances: HashMap<D::Id, HashMap<OperatorId, RunningInstance<T, D>>>,
    id: Option<D::Id>,
    tmp: Vec<Arc<QbftManager<T>>>,
    managers: HashMap<OperatorId, Arc<QbftManager<T>>>,
    // Track consensus results
    successful_instances: HashSet<D::Id>,
    failed_instances: HashSet<D::Id>,
    // Channel for QBFT network messages
    network_rx: UnboundedReceiver<Message>,
    network_tx: UnboundedSender<Message>,
    // Slot clock for timing
    slot_clock: T,
    size: CommitteeSize,
}

impl<T, D> QbftTester<T, D>
where
    T: SlotClock + 'static,
    D: QbftDecidable<T> + 'static,
{
    /// Create a new QBFT tester instance
    pub fn new(slot_clock: T, executor: TaskExecutor, size: CommitteeSize) -> Self {
        // Setup the processor
        let config = processor::Config { max_workers: 15 };
        let sender_queues = processor::spawn(config, executor);

        // Simulate the network sender and receiver. Qbft instances will send UnsignedSSVMessages
        // out on the network_tx and they will be recieved by the network_rx to be "signed" and then
        // multicast broadcasted back into the instances for simulation
        let (network_tx, network_rx) = mpsc::unbounded_channel();

        // Construct and save a manager for each operator in the committee. By having access to all
        // the managers in the committee, we can properly direct messages to the proper place and
        // spawn multiple concurrent instances
        let mut managers = HashMap::new();
        for id in 1..=(size as u64) {
            let operator_id = OperatorId(id);
            let manager = QbftManager::new(
                sender_queues.clone(),
                operator_id,
                slot_clock.clone(),
                network_tx.clone(),
            )
            .expect("Creation should not fail");

            managers.insert(operator_id, manager);
        }

        Self {
            senders: sender_queues,
            id: None,
            instances: HashMap::new(),
            managers,
            tmp: Vec::new(),
            successful_instances: HashSet::new(),
            failed_instances: HashSet::new(),
            network_rx,
            network_tx,
            slot_clock,
            size
        }
    }

    // Start a new full test instance for the provided configuration. This will start a new qbft
    // instance for each operator in the committee. This simulates distributed instances each
    // starting their own instance when they must reach consensus with the rest of the committee
    pub async fn start_instance(
        &mut self,
        config: TestInstanceConfig<T, D>,
    ) -> Result<(), QbftError> {

        // Dummy cluster
        let cluster = Cluster {
            cluster_id: ClusterId([0; 32]),
            owner: Default::default(),
            fee_recipient: Default::default(),
            faulty: self.size.get_f(),
            liquidated: false,
            cluster_members: (1..=(self.size as u64)).map(OperatorId).collect(),
        };


        // Go through all of the managers. Spawn a new instance for the data and record it
        for (id, manager) in &self.managers {
            let manager_clone = manager.clone();
            let cluster = cluster.clone();
            let data = config.initial_data.clone();
            let id = config.id.clone();
            let _ = self.senders.permitless.send_async(
                async move {
                    let result = manager_clone.decide_instance(id, data, &cluster).await;
                    println!("{:?}", result);
                },
                "qbft_instance starting",
            );

            // todo!() track some unique footprint
        }
        Ok(())
    }

    // If I want to simulate running multipel instances
    // Each instance needs some unique id, this is just to say like hash 10 -> idx 0xsdasdfasdf
    // There will always be commit size managers
    // We can run multiple consensus intances at once
    // so just keep calling decide instance, but, i need some sort of id to differential betweent
    // hem. also, how can I tell when they are all done? Soemthing like
    // i also need to be able to reuse the instances, I dont want to create like 20managers. just
    // num_commitee managers

    // When all the instances are spawned, handle all outgoing messages
    pub async fn run_until_complete(&mut self) -> ConsensusResult {
        while let Some(msg) = self.network_rx.recv().await {
            // Extract operator ID and message content
            let (sender_operator_id, unsigned_msg) = match msg {
                Message::Propose(id, msg) => (id, msg),
                Message::Prepare(id, msg) => (id, msg),
                Message::Commit(id, msg) => (id, msg),
                Message::RoundChange(id, msg) => (id, msg),
            };

            // First decode the QBFT message to get the instance identifier
            let qbft_msg = match QbftMessage::from_ssz_bytes(unsigned_msg.ssv_message.data()) {
                Ok(msg) => msg,
                Err(_) => {
                    return;
                }
            };

            // Create wrapped message
            let signed_msg = SignedSSVMessage::new(
                vec![vec![0; 96]], // Test signature
                vec![*sender_operator_id],
                unsigned_msg.ssv_message.clone(),
                unsigned_msg.full_data,
            )
            .expect("Failed to create signed message");

            let wrapped_msg = WrappedQbftMessage {
                signed_message: signed_msg,
                qbft_message: qbft_msg.clone(),
            };

            let id = self.id.clone().unwrap();
            for manager in &self.tmp {
                let _ = manager.receive_data::<D>(id.clone(), wrapped_msg.clone());
            }
        }
        todo!()
    }

    pub async fn handle_network_message(&mut self, msg: Message) {

        // broadcast this message to all of the other instances
    }
}

pub struct ConsensusResult {
    reached_consensus: bool,
}

#[cfg(test)]
mod manager_tests {
    use super::*;

    // Provides test setup
    struct Setup {
        executor: TaskExecutor,
        _signal: async_channel::Sender<()>,
        _shutdown: futures::channel::mpsc::Sender<ShutdownReason>,
        clock: SystemTimeSlotClock,
        data: BeaconVote,
        id: CommitteeInstanceId,
    }

    fn setup_test() -> Setup {
        let env_filter = tracing_subscriber::EnvFilter::new("debug");
        tracing_subscriber::fmt().with_env_filter(env_filter).init();

        // setup the executor
        let handle = tokio::runtime::Handle::current();
        let (signal, exit) = async_channel::bounded(1);
        let (shutdown, _) = futures::channel::mpsc::channel(1);
        let executor = TaskExecutor::new(handle, exit, shutdown.clone());

        // setup the slot clock
        let slot_duration = Duration::from_secs(12);
        let genesis_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let clock = SystemTimeSlotClock::new(
            Slot::new(0),
            Duration::from_secs(genesis_time),
            slot_duration,
        );

        // setup mock data
        let id = CommitteeInstanceId {
            committee: ClusterId([0; 32]),
            instance_height: 10.into(),
        };

        let data = BeaconVote {
            block_root: Hash256::default(),
            source: types::Checkpoint::default(),
            target: types::Checkpoint::default(),
        };

        Setup {
            executor,
            _signal: signal,
            _shutdown: shutdown,
            clock,
            data,
            id,
        }
    }

    #[tokio::test]
    // Test running a single instance and confirm that it reaches consensus
    async fn test_basic_run() {
        // Standard setup
        let setup = setup_test();

        // Setup the tester
        let mut tester: QbftTester<SystemTimeSlotClock, BeaconVote> =
            QbftTester::new(setup.clock, setup.executor, CommitteeSize::Four);

        // Create instance configuration data and then start the instance
        let instance_config = TestInstanceConfigBuilder::new()
            .with_data(setup.data)
            .with_id(setup.id)
            .build();

        tester
            .start_instance(instance_config)
            .await
            .expect("Should start instance");

        // Wait for it to run and get the result
        let result = tester.run_until_complete().await;

        // Confirm that we reached consensus
        assert!(result.reached_consensus);
    }
}
