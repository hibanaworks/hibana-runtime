//! Each localside receives its already-issued Endpoint and writes operations
//! and application effects directly. It never defines or projects the global.
use super::protocol::{AlternatePing, AlternatePong, Ping, Pong, Read, Value};
use core::cell::Cell;
use hibana::{Endpoint, EndpointError};

pub async fn left(
    mut endpoint: Endpoint<'_, 0>,
    alternate: bool,
    result: &Cell<u32>,
) -> Result<(), EndpointError> {
    let reply = if alternate {
        endpoint.send::<AlternatePing>(&70).await?;
        endpoint.recv::<AlternatePong>().await?
    } else {
        endpoint.send::<Ping>(&7).await?;
        endpoint.recv::<Pong>().await?
    };
    result.set(reply);
    Ok(())
}

pub async fn left_peer(mut endpoint: Endpoint<'_, 1>) -> Result<(), EndpointError> {
    let branch = endpoint.offer().await?;
    match branch.label() {
        1 => {
            let value = branch.recv::<Ping>().await?;
            endpoint.send::<Pong>(&(value + 1)).await
        }
        3 => {
            let value = branch.recv::<AlternatePing>().await?;
            endpoint.send::<AlternatePong>(&(value + 1)).await
        }
        _ => unreachable!("the global has exactly two route arms"),
    }
}

pub async fn right(mut endpoint: Endpoint<'_, 2>, result: &Cell<u32>) -> Result<(), EndpointError> {
    endpoint.send::<Read>(&42).await?;
    result.set(endpoint.recv::<Value>().await?);
    Ok(())
}

pub async fn right_peer(mut endpoint: Endpoint<'_, 3>) -> Result<(), EndpointError> {
    let key = endpoint.recv::<Read>().await?;
    endpoint.send::<Value>(&(key * 2)).await
}
