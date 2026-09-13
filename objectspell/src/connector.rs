// Connector module.
use crate::AnyState;

/// What `connect()` accepts: a component, or a tuple of up to sixteen of them.
pub trait Connectable {
    /// Turn each component into a runnable state and collect them.
    fn into_states(self, vec: &mut Vec<Box<dyn AnyState>>);
}

macro_rules! impl_connectable_tuple {
    ($($T:ident),+) => {
        impl<$($T: Connectable),+> Connectable for ($($T,)+) {
            #[allow(non_snake_case)]
            fn into_states(self, vec: &mut Vec<Box<dyn AnyState>>) {
                let ($($T,)+) = self;
                $($T.into_states(vec);)+
            }
        }
    };
}

impl_connectable_tuple!(T1);
impl_connectable_tuple!(T1, T2);
impl_connectable_tuple!(T1, T2, T3);
impl_connectable_tuple!(T1, T2, T3, T4);
impl_connectable_tuple!(T1, T2, T3, T4, T5);
impl_connectable_tuple!(T1, T2, T3, T4, T5, T6);
impl_connectable_tuple!(T1, T2, T3, T4, T5, T6, T7);
impl_connectable_tuple!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_connectable_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_connectable_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_connectable_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_connectable_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_connectable_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
impl_connectable_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
impl_connectable_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
impl_connectable_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

impl Connectable for () {
    fn into_states(self, _vec: &mut Vec<Box<dyn AnyState>>) {}
}

#[objectspell::emitter]
pub trait Connector {
    async fn connected();
    async fn disconnected();
}

/// Starts and stops a topology.
///
/// `connect` checks the wiring, starts every component, and emits `connected`. It does not
/// return until the topology has shut down, so it is normally the whole program.
#[objectspell::state]
pub struct Connector {}

impl Connector {
    /// Create a Connector.
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    /// Check the wiring, connect every State to the emitters it listens to, start them, and
    /// emit `connected`.
    ///
    /// Does not return until the topology has shut down, so it is normally the whole program.
    /// A topology whose wiring is wrong is rejected before anything starts.
    pub async fn connect<C: Connectable>(self, states_tuple: C) -> Result<(), crate::WiringError> {
        let connector = self.into_state();

        let mut states_vec = Vec::new();
        states_tuple.into_states(&mut states_vec);

        let mut states_slice_vec: Vec<&dyn AnyState> = vec![connector.as_ref()];
        states_slice_vec.extend(states_vec.iter().map(|s| s.as_ref()));
        let states_slice = states_slice_vec.as_slice();

        // 0. Nothing has been wired or started yet, so a rejected topology leaves no trace.
        crate::wiring::validate(states_slice).await?;

        // 1. Collect all senders and channel names for receivers
        let mut receiver_info = Vec::new();
        for state in states_slice {
            if let Some(tx) = state.sender().await {
                let channels = state.channels().await;
                receiver_info.push((tx, channels));
            }
        }

        // 2. Wire all senders to all compatible emitters
        for state in states_slice {
            let emitter_name = state.emitter_name().await;
            for (tx, channels) in &receiver_info {
                if channels.contains(&emitter_name) {
                    state.connect_receiver(tx.clone()).await;
                }
            }
        }

        // 3. Start listeners
        let mut handles = Vec::new();
        for state in states_slice {
            if let Some(handle) = state.start_listener().await {
                handles.push(handle);
            }
        }

        // 4. Trigger "connected" lifecycle event on the Connector
        let channel = connector.emitter_name().await;
        connector
            .broadcast(crate::Signal::new(channel, "connected"))
            .await;

        for handle in handles {
            let _ = handle.await;
        }

        Ok(())
    }

    /// Emit `disconnected`. Every listening State handles it, so this stops all of them.
    pub async fn disconnect(&self) {
        self.disconnected().await;
    }
}
