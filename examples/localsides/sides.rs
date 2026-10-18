//! Each localside receives its already-issued Endpoint and writes operations
//! and application effects directly. It never defines or projects the global.
use super::protocol::{
    AUDITOR, CLIENT, Count, FETCH, Fetch, Fetched, Inspect, SERVER, STORE, UPDATE, Update, Updated,
};
use core::cell::Cell;
use hibana::{Endpoint, EndpointError};
use hibana_runtime::localside;

#[localside]
pub async fn client(
    mut client: Endpoint<CLIENT>,
    update: bool,
    result: &Cell<u32>,
) -> Result<(), EndpointError> {
    let reply = if update {
        client.send::<Update>(&70).await?;
        client.recv::<Updated>().await?
    } else {
        client.send::<Fetch>(&7).await?;
        client.recv::<Fetched>().await?
    };
    result.set(reply);
    Ok(())
}

#[localside]
pub async fn server(mut server: Endpoint<SERVER>) -> Result<(), EndpointError> {
    let branch = server.offer().await?;
    match branch.label() {
        FETCH => {
            let value = branch.recv::<Fetch>().await?;
            server.send::<Fetched>(&(value + 1)).await
        }
        UPDATE => {
            let value = branch.recv::<Update>().await?;
            server.send::<Updated>(&(value + 1)).await
        }
        _ => unreachable!("the global has exactly two route arms"),
    }
}

#[localside]
pub async fn auditor(
    mut auditor: Endpoint<AUDITOR>,
    result: &Cell<u32>,
) -> Result<(), EndpointError> {
    auditor.send::<Inspect>(&42).await?;
    result.set(auditor.recv::<Count>().await?);
    Ok(())
}

#[localside]
pub async fn store(mut store: Endpoint<STORE>) -> Result<(), EndpointError> {
    let key = store.recv::<Inspect>().await?;
    store.send::<Count>(&(key * 2)).await
}
