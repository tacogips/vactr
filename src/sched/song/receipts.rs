//! Full-key receipt dispatch keeps unrelated owners' records intact.
use super::*;
pub(super) fn receive(transport: &mut SongTransport, ack: SongHostAck) -> Result<(), SongHostAck> {
    if ack.epoch() != transport.epoch {
        return Err(ack);
    }
    if let SongHostAck::BranchRebound(rebound) = ack {
        return if transport.pools.receive(rebound) {
            Ok(())
        } else {
            Err(ack)
        };
    }
    if let SongHostAck::Rejected { .. } = ack {
        transport.fail_and_retire(failure("owned song runtime command rejected"));
        return Ok(());
    }
    if let Some(ready) = &mut transport.ready {
        ready.receive_activation(ack)?;
        if ready.activation_rejection().is_some() {
            transport.failure = Some(failure("song activation rejected"));
            transport.state = SongTransportState::Failed;
            transport.pending.clear();
            transport.begin_retirement();
        } else if ready.applied_activation().is_some() {
            transport.applied = ready.applied_activation();
            if transport.failure.is_none() {
                transport.state = SongTransportState::Playing;
            }
        }
        return Ok(());
    }
    if let Some(cleanup) = &mut transport.cleanup {
        cleanup.receive(ack)?;
        match ack {
            SongHostAck::Applied(actual) => {
                transport.applied = Some(actual);
                if transport.failure.is_none() {
                    transport.state = SongTransportState::Playing;
                }
            }
            SongHostAck::ActivationRejected { .. } => {
                transport.failure = Some(failure("song activation rejected"));
                transport.state = SongTransportState::Failed;
            }
            _ => {}
        }
        Ok(())
    } else {
        Err(ack)
    }
}
