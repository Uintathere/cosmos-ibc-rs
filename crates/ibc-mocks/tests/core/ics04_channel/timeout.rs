use ibc::core::events::{IbcEvent, MessageEvent};
use ibc::core::ics02_client::height::Height;
use ibc::core::ics02_client::ClientExecutionContext;
use ibc::core::ics03_connection::connection::{
    ConnectionEnd, Counterparty as ConnectionCounterparty, State as ConnectionState,
};
use ibc::core::ics03_connection::version::get_compatible_versions;
use ibc::core::ics04_channel::channel::{ChannelEnd, Counterparty, Order, State};
use ibc::core::ics04_channel::commitment::{compute_packet_commitment, PacketCommitment};
use ibc::core::ics04_channel::msgs::timeout::test_util::get_dummy_raw_msg_timeout;
use ibc::core::ics04_channel::msgs::timeout::MsgTimeout;
use ibc::core::ics04_channel::msgs::PacketMsg;
use ibc::core::ics04_channel::Version;
use ibc::core::ics24_host::identifier::{ChannelId, ClientId, ConnectionId, PortId};
use ibc::core::timestamp::{Timestamp, ZERO_DURATION};
use ibc::core::{execute, validate, ExecutionContext, MsgEnvelope};
use ibc::prelude::*;
use ibc_mocks::core::definition::MockContext;
use ibc_mocks::router::definition::MockRouter;
use rstest::*;

struct Fixture {
    ctx: MockContext,
    pub router: MockRouter,
    client_height: Height,
    msg: MsgTimeout,
    packet_commitment: PacketCommitment,
    conn_end_on_a: ConnectionEnd,
    chan_end_on_a_ordered: ChannelEnd,
    chan_end_on_a_unordered: ChannelEnd,
}

#[fixture]
fn fixture() -> Fixture {
    let client_height = Height::new(0, 2).unwrap();
    let ctx = MockContext::default().with_client(&ClientId::default(), client_height);

    let client_height = Height::new(0, 2).unwrap();

    let router = MockRouter::new_with_transfer();

    let msg_proof_height = 2;
    let msg_timeout_height = 5;
    let timeout_timestamp = Timestamp::now().nanoseconds();

    let msg = MsgTimeout::try_from(get_dummy_raw_msg_timeout(
        msg_proof_height,
        msg_timeout_height,
        timeout_timestamp,
    ))
    .unwrap();

    let packet = msg.packet.clone();

    let packet_commitment = compute_packet_commitment(
        &msg.packet.data,
        &msg.packet.timeout_height_on_b,
        &msg.packet.timeout_timestamp_on_b,
    );

    let chan_end_on_a_unordered = ChannelEnd::new(
        State::Open,
        Order::Unordered,
        Counterparty::new(packet.port_id_on_b.clone(), Some(packet.chan_id_on_b)),
        vec![ConnectionId::default()],
        Version::new("ics20-1".to_string()),
    )
    .unwrap();

    let mut chan_end_on_a_ordered = chan_end_on_a_unordered.clone();
    chan_end_on_a_ordered.ordering = Order::Ordered;

    let conn_end_on_a = ConnectionEnd::new(
        ConnectionState::Open,
        ClientId::default(),
        ConnectionCounterparty::new(
            ClientId::default(),
            Some(ConnectionId::default()),
            Default::default(),
        ),
        get_compatible_versions(),
        ZERO_DURATION,
    )
    .unwrap();

    Fixture {
        ctx,
        router,
        client_height,
        msg,
        packet_commitment,
        conn_end_on_a,
        chan_end_on_a_ordered,
        chan_end_on_a_unordered,
    }
}

#[rstest]
fn timeout_fail_no_channel(fixture: Fixture) {
    let Fixture {
        ctx,
        router,
        msg,
        client_height,
        ..
    } = fixture;
    let ctx = ctx.with_client(&ClientId::default(), client_height);
    let msg_envelope = MsgEnvelope::from(PacketMsg::from(msg));
    let res = validate(&ctx, &router, msg_envelope);

    assert!(
        res.is_err(),
        "Validation fails because no channel exists in the context"
    )
}

#[rstest]
fn timeout_fail_no_consensus_state_for_height(fixture: Fixture) {
    let Fixture {
        ctx,
        router,
        msg,
        chan_end_on_a_unordered,
        conn_end_on_a,
        packet_commitment,
        ..
    } = fixture;

    let packet = msg.packet.clone();

    let ctx = ctx
        .with_channel(
            PortId::transfer(),
            ChannelId::default(),
            chan_end_on_a_unordered,
        )
        .with_connection(ConnectionId::default(), conn_end_on_a)
        .with_packet_commitment(
            packet.port_id_on_a,
            packet.chan_id_on_a,
            packet.seq_on_a,
            packet_commitment,
        );

    let msg_envelope = MsgEnvelope::from(PacketMsg::from(msg));

    let res = validate(&ctx, &router, msg_envelope);

    assert!(
            res.is_err(),
            "Validation fails because the client does not have a consensus state for the required height"
        )
}

#[rstest]
fn timeout_fail_proof_timeout_not_reached(fixture: Fixture) {
    let Fixture {
        ctx,
        router,
        mut msg,
        chan_end_on_a_unordered,
        conn_end_on_a,
        client_height,
        ..
    } = fixture;

    // timeout timestamp has not reached yet
    let timeout_timestamp_on_b =
        (msg.packet.timeout_timestamp_on_b + core::time::Duration::new(10, 0)).unwrap();
    msg.packet.timeout_timestamp_on_b = timeout_timestamp_on_b;
    let packet_commitment = compute_packet_commitment(
        &msg.packet.data,
        &msg.packet.timeout_height_on_b,
        &msg.packet.timeout_timestamp_on_b,
    );

    let packet = msg.packet.clone();

    let mut ctx = ctx
        .with_client(&ClientId::default(), client_height)
        .with_connection(ConnectionId::default(), conn_end_on_a)
        .with_channel(
            PortId::transfer(),
            ChannelId::default(),
            chan_end_on_a_unordered,
        )
        .with_packet_commitment(
            packet.port_id_on_a,
            packet.chan_id_on_a,
            packet.seq_on_a,
            packet_commitment,
        );

    ctx.store_update_time(
        ClientId::default(),
        client_height,
        Timestamp::from_nanoseconds(5).unwrap(),
    )
    .unwrap();
    ctx.store_update_height(
        ClientId::default(),
        client_height,
        Height::new(0, 4).unwrap(),
    )
    .unwrap();

    let msg_envelope = MsgEnvelope::from(PacketMsg::from(msg));

    let res = validate(&ctx, &router, msg_envelope);

    assert!(
            res.is_err(),
            "Validation should fail because the timeout height was reached, but the timestamp hasn't been reached. Both the height and timestamp need to be reached for the packet to be considered timed out"
        )
}

/// NO-OP case
#[rstest]
fn timeout_success_no_packet_commitment(fixture: Fixture) {
    let Fixture {
        ctx,
        router,
        msg,
        conn_end_on_a,
        chan_end_on_a_unordered,
        ..
    } = fixture;
    let ctx = ctx
        .with_channel(
            PortId::transfer(),
            ChannelId::default(),
            chan_end_on_a_unordered,
        )
        .with_connection(ConnectionId::default(), conn_end_on_a);

    let msg_envelope = MsgEnvelope::from(PacketMsg::from(msg));

    let res = validate(&ctx, &router, msg_envelope);

    assert!(
        res.is_ok(),
        "Validation should succeed when no packet commitment is present"
    )
}

#[rstest]
fn timeout_unordered_channel_validate(fixture: Fixture) {
    let Fixture {
        ctx,
        router,
        msg,
        chan_end_on_a_unordered,
        conn_end_on_a,
        packet_commitment,
        client_height,
        ..
    } = fixture;

    let packet = msg.packet.clone();

    let mut ctx = ctx
        .with_client(&ClientId::default(), client_height)
        .with_connection(ConnectionId::default(), conn_end_on_a)
        .with_channel(
            PortId::transfer(),
            ChannelId::default(),
            chan_end_on_a_unordered,
        )
        .with_packet_commitment(
            packet.port_id_on_a,
            packet.chan_id_on_a,
            packet.seq_on_a,
            packet_commitment,
        );

    ctx.get_client_execution_context()
        .store_update_time(
            ClientId::default(),
            client_height,
            Timestamp::from_nanoseconds(1000).unwrap(),
        )
        .unwrap();
    ctx.get_client_execution_context()
        .store_update_height(
            ClientId::default(),
            client_height,
            Height::new(0, 5).unwrap(),
        )
        .unwrap();

    let msg_envelope = MsgEnvelope::from(PacketMsg::from(msg));

    let res = validate(&ctx, &router, msg_envelope);

    assert!(res.is_ok(), "Good parameters for unordered channels")
}

#[rstest]
fn timeout_ordered_channel_validate(fixture: Fixture) {
    let Fixture {
        ctx,
        router,
        msg,
        chan_end_on_a_ordered,
        conn_end_on_a,
        packet_commitment,
        client_height,
        ..
    } = fixture;

    let packet = msg.packet.clone();

    let mut ctx = ctx
        .with_client(&ClientId::default(), client_height)
        .with_connection(ConnectionId::default(), conn_end_on_a)
        .with_channel(
            PortId::transfer(),
            ChannelId::default(),
            chan_end_on_a_ordered,
        )
        .with_packet_commitment(
            packet.port_id_on_a,
            packet.chan_id_on_a,
            packet.seq_on_a,
            packet_commitment,
        );

    ctx.store_update_time(
        ClientId::default(),
        client_height,
        Timestamp::from_nanoseconds(1000).unwrap(),
    )
    .unwrap();
    ctx.store_update_height(
        ClientId::default(),
        client_height,
        Height::new(0, 4).unwrap(),
    )
    .unwrap();

    let msg_envelope = MsgEnvelope::from(PacketMsg::from(msg));

    let res = validate(&ctx, &router, msg_envelope);

    assert!(res.is_ok(), "Good parameters for unordered channels")
}

#[rstest]
fn timeout_unordered_chan_execute(fixture: Fixture) {
    let Fixture {
        ctx,
        mut router,
        msg,
        packet_commitment,
        conn_end_on_a,
        chan_end_on_a_unordered,
        ..
    } = fixture;
    let mut ctx = ctx
        .with_channel(
            PortId::transfer(),
            ChannelId::default(),
            chan_end_on_a_unordered,
        )
        .with_connection(ConnectionId::default(), conn_end_on_a)
        .with_packet_commitment(
            msg.packet.port_id_on_a.clone(),
            msg.packet.chan_id_on_a.clone(),
            msg.packet.seq_on_a,
            packet_commitment,
        );

    let msg_envelope = MsgEnvelope::from(PacketMsg::from(msg));

    let res = execute(&mut ctx, &mut router, msg_envelope);

    assert!(res.is_ok());

    // Unordered channels only emit one event
    assert_eq!(ctx.events.len(), 2);
    assert!(matches!(
        ctx.events[0],
        IbcEvent::Message(MessageEvent::Channel)
    ));
    assert!(matches!(ctx.events[1], IbcEvent::TimeoutPacket(_)));
}

#[rstest]
fn timeout_ordered_chan_execute(fixture: Fixture) {
    let Fixture {
        ctx,
        mut router,
        msg,
        packet_commitment,
        conn_end_on_a,
        chan_end_on_a_ordered,
        ..
    } = fixture;
    let mut ctx = ctx
        .with_channel(
            PortId::transfer(),
            ChannelId::default(),
            chan_end_on_a_ordered,
        )
        .with_connection(ConnectionId::default(), conn_end_on_a)
        .with_packet_commitment(
            msg.packet.port_id_on_a.clone(),
            msg.packet.chan_id_on_a.clone(),
            msg.packet.seq_on_a,
            packet_commitment,
        );

    let msg_envelope = MsgEnvelope::from(PacketMsg::from(msg));

    let res = execute(&mut ctx, &mut router, msg_envelope);

    assert!(res.is_ok());

    // Ordered channels emit 2 events
    assert_eq!(ctx.events.len(), 4);
    assert!(matches!(
        ctx.events[0],
        IbcEvent::Message(MessageEvent::Channel)
    ));
    assert!(matches!(ctx.events[1], IbcEvent::TimeoutPacket(_)));
    assert!(matches!(
        ctx.events[2],
        IbcEvent::Message(MessageEvent::Channel)
    ));
    assert!(matches!(ctx.events[3], IbcEvent::ChannelClosed(_)));
}
