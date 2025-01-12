use bevy::prelude::*;
use bytes::Bytes;

use crate::core::ClientId;

/// Stores information about a client independent from the messaging backend.
///
/// The messaging backend is responsible for updating this resource:
/// - When the messaging client changes its status (connected, connecting and disconnected),
/// [`Self::set_status`] should be used to reflect this.
/// - For receiving messages, [`Self::insert_received`] should be to used.
/// A system to forward backend messages to Replicon should run in
/// [`ClientSet::ReceivePackets`](super::ClientSet::ReceivePackets).
/// - For sending messages, [`Self::drain_sent`] should be used to drain all sent messages.
/// A system to forward Replicon messages to the backend should run in
/// [`ClientSet::SendPackets`](super::ClientSet::SendPackets).
#[derive(Resource, Default)]
pub struct RepliconClient {
    /// Client connection status.
    status: RepliconClientStatus,

    /// List of received messages for each channel.
    ///
    /// Top index is channel ID.
    /// Inner [`Vec`] stores received messages since the last tick.
    received_messages: Vec<Vec<Bytes>>,

    /// List of sent messages and their channels since the last tick.
    sent_messages: Vec<(u8, Bytes)>,
}

impl RepliconClient {
    /// Changes the size of the receive messages storage according to the number of server channels.
    pub(super) fn setup_server_channels(&mut self, channels_count: usize) {
        self.received_messages.resize(channels_count, Vec::new());
    }

    /// Pops the next available message from the server over a channel.
    pub fn receive<I: Into<u8>>(&mut self, channel_id: I) -> Option<Bytes> {
        if !self.is_connected() {
            warn!("trying to receive a message when the client is not connected");
            return None;
        }

        let channel_id = channel_id.into();
        let channel_messages = self
            .received_messages
            .get_mut(channel_id as usize)
            .unwrap_or_else(|| panic!("client should have a receive channel with id {channel_id}"));

        channel_messages.pop()
    }

    /// Sends a message to the server over a channel.
    pub fn send<I: Into<u8>, B: Into<Bytes>>(&mut self, channel_id: I, message: B) {
        if !self.is_connected() {
            warn!("trying to send a message when the client is not connected");
            return;
        }

        self.sent_messages.push((channel_id.into(), message.into()));
    }

    /// Sets the client connection status.
    ///
    /// Should be called only from the messaging backend when the client status changes.
    /// Discards all messages if the state changes from [`RepliconClientStatus::Connected`].
    /// See also [`Self::status`].
    pub fn set_status(&mut self, status: RepliconClientStatus) {
        debug!("changing `RepliconClient` status to `{status:?}`");

        if self.is_connected() && !matches!(status, RepliconClientStatus::Connected { .. }) {
            for channel_messages in &mut self.received_messages {
                channel_messages.clear();
            }
            self.sent_messages.clear();
        }

        self.status = status;
    }

    /// Returns the current client status.
    ///
    /// See also [`Self::set_status`].
    #[inline]
    pub fn status(&self) -> RepliconClientStatus {
        self.status
    }

    /// Returns `true` if the client is disconnected.
    ///
    /// See also [`Self::status`].
    #[inline]
    pub fn is_disconnected(&self) -> bool {
        matches!(self.status, RepliconClientStatus::Disconnected)
    }

    /// Returns `true` if the client is connecting.
    ///
    /// See also [`Self::status`].
    #[inline]
    pub fn is_connecting(&self) -> bool {
        matches!(self.status, RepliconClientStatus::Connecting)
    }

    /// Returns `true` if the client is connected.
    ///
    /// See also [`Self::status`].
    #[inline]
    pub fn is_connected(&self) -> bool {
        matches!(self.status, RepliconClientStatus::Connected { .. })
    }

    /// Returns the client's ID.
    ///
    /// The client ID is available only if the client state is [`RepliconClientStatus::Connected`].
    /// See also [`Self::status`].
    #[inline]
    pub fn id(&self) -> Option<ClientId> {
        if let RepliconClientStatus::Connected { client_id } = self.status {
            client_id
        } else {
            None
        }
    }

    /// Removes all sent messages, returning them as an iterator with channel.
    ///
    /// Should be called only from the messaging backend.
    pub fn drain_sent(&mut self) -> impl Iterator<Item = (u8, Bytes)> + '_ {
        self.sent_messages.drain(..)
    }

    /// Adds a message from the server to the list of received messages.
    ///
    /// Should be called only from the messaging backend.
    pub fn insert_received<I: Into<u8>, B: Into<Bytes>>(&mut self, channel_id: I, message: B) {
        if !self.is_connected() {
            warn!("trying to insert a received message when the client is not connected");
            return;
        }

        let channel_id = channel_id.into();
        let channel_messages = self
            .received_messages
            .get_mut(channel_id as usize)
            .unwrap_or_else(|| panic!("client should have a channel with id {channel_id}"));

        channel_messages.push(message.into());
    }
}

/// Connection status of the [`RepliconClient`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RepliconClientStatus {
    /// Not connected or trying to connect.
    #[default]
    Disconnected,
    /// Trying to connect to the server.
    Connecting,
    /// Connected to the server.
    ///
    /// Stores the assigned ID if one was assigned by the server.
    /// Needed only for users to access ID independent from messaging library.
    Connected { client_id: Option<ClientId> },
}
