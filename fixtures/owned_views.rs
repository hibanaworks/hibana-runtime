//! Internal integration evidence against the development core's real kernel.
//! The public owned-projection guard stays enabled. Like the core's own owned
//! tests, this attaches its production-lowered private image for execution.
use super::{Loan, OtherLoan, Read, RoleProjection, Value};
use crate::{
    Endpoint, EndpointError, g,
    runtime::{SessionKitStorage, ids::SessionId},
    test_support::transport::TestTransport,
};
use core::{
    cell::Cell,
    task::{Poll, Waker},
};
use hibana_runtime::{Driver, localside};

type Outcome<T = ()> = Result<T, EndpointError>;

struct TestEvents;
impl Driver for TestEvents {
    fn waker(&self) -> Waker {
        Waker::noop().clone()
    }
    fn wait(&mut self) {}
}

async fn yield_once() {
    let mut yielded = false;
    core::future::poll_fn(|cx| {
        if yielded {
            Poll::Ready(())
        } else {
            yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await;
}

#[localside]
async fn worker(mut worker: Endpoint<'_, 1>, effects: &Cell<u32>) -> Outcome {
    let mut db = worker.recv::<Loan>().await?;
    let mut auth = worker.recv::<OtherLoan>().await?;
    let db_started = Cell::new(false);
    let auth_started = Cell::new(false);
    {
        async {
            // Deliberately poll the second source arm first.
            auth.send::<Read>(&11).await?;
            auth_started.set(true);
            while !db_started.get() {
                yield_once().await;
            }
            assert_eq!(auth.recv::<Value>().await?, 12);
            effects.set(effects.get() + 1);
        };
        async {
            db.send::<Read>(&7).await?;
            db_started.set(true);
            while !auth_started.get() {
                yield_once().await;
            }
            assert_eq!(db.recv::<Value>().await?, 8);
            effects.set(effects.get() + 1);
        };
    }
    // Both borrows have ended. Return the actual owned views, not recreated
    // handles, and let OWNER continue from their current protocol position.
    worker.send::<Loan>(db).await?;
    worker.send::<OtherLoan>(auth).await
}

#[localside]
async fn owner(
    mut owner: Endpoint<'_, 0>,
    db: Endpoint<'_, 10>,
    auth: Endpoint<'_, 11>,
) -> Outcome {
    owner.send::<Loan>(db).await?;
    owner.send::<OtherLoan>(auth).await?;
    let mut db = owner.recv::<Loan>().await?;
    let mut auth = owner.recv::<OtherLoan>().await?;
    {
        async {
            db.send::<Read>(&100).await?;
            assert_eq!(db.recv::<Value>().await?, 101);
        };
        async {
            auth.send::<Read>(&200).await?;
            assert_eq!(auth.recv::<Value>().await?, 201);
        };
    }
    Ok(())
}

#[localside]
async fn service<const ROLE: u8>(mut endpoint: Endpoint<'_, ROLE>) -> Outcome {
    for _ in 0..2 {
        let request = endpoint.recv::<Read>().await?;
        yield_once().await;
        endpoint.send::<Value>(&(request + 1)).await?;
    }
    Ok(())
}

#[test]
fn owned_views_cross_actor_runtimes_and_continue_after_their_return() {
    #[hibana_runtime::main(TestEvents)]
    async fn execute() -> Outcome {
        type Work = g::Par<
            g::Seq<g::Send<10, 3, Read>, g::Send<3, 10, Value>>,
            g::Seq<g::Send<11, 4, Read>, g::Send<4, 11, Value>>,
        >;
        type Steps = g::Seq<
            g::Send<0, 1, Loan>,
            g::Seq<
                g::Send<0, 1, OtherLoan>,
                g::Seq<Work, g::Seq<g::Send<1, 0, Loan>, g::Seq<g::Send<1, 0, OtherLoan>, Work>>>,
            >,
        >;
        let owner_program = crate::global::role_program::role_program_from_image::<0>(
            &RoleProjection::<0, Steps, 32>::IMAGE_REF,
        );
        let worker_program = crate::global::role_program::role_program_from_image::<1>(
            &RoleProjection::<1, Steps, 32>::IMAGE_REF,
        );
        let db_program = crate::global::role_program::role_program_from_image::<10>(
            &RoleProjection::<10, Steps, 32>::IMAGE_REF,
        );
        let auth_program = crate::global::role_program::role_program_from_image::<11>(
            &RoleProjection::<11, Steps, 32>::IMAGE_REF,
        );
        let db_service_program = crate::global::role_program::role_program_from_image::<3>(
            &RoleProjection::<3, Steps, 32>::IMAGE_REF,
        );
        let auth_service_program = crate::global::role_program::role_program_from_image::<4>(
            &RoleProjection::<4, Steps, 32>::IMAGE_REF,
        );

        let transport = TestTransport::new();
        let mut owner_slab = [0; 64 * 1024];
        let mut worker_slab = [0; 64 * 1024];
        let mut service_slab = [0; 64 * 1024];
        let mut owner_storage = SessionKitStorage::<TestTransport>::uninit();
        let mut worker_storage = SessionKitStorage::<TestTransport>::uninit();
        let mut service_storage = SessionKitStorage::<TestTransport>::uninit();
        let owner_rv = owner_storage
            .init()
            .rendezvous(&mut owner_slab, transport.clone())
            .unwrap();
        let worker_rv = worker_storage
            .init()
            .rendezvous(&mut worker_slab, transport.clone())
            .unwrap();
        let service_rv = service_storage
            .init()
            .rendezvous(&mut service_slab, transport)
            .unwrap();
        let sid = SessionId::new(129);
        let owner_view = owner_rv.enter(sid, &owner_program).unwrap();
        let db = owner_rv.enter(sid, &db_program).unwrap();
        let auth = owner_rv.enter(sid, &auth_program).unwrap();
        let worker_view = worker_rv.enter(sid, &worker_program).unwrap();
        let db_service = service_rv.enter(sid, &db_service_program).unwrap();
        let auth_service = service_rv.enter(sid, &auth_service_program).unwrap();
        let effects = Cell::new(0);
        {
            #[localside]
            service(db_service);
            #[localside]
            service(auth_service);
            #[localside]
            worker(worker_view, &effects);
            #[localside]
            owner(owner_view, db, auth);
        }
        assert_eq!(effects.get(), 2);
        Ok(())
    }
    execute().unwrap();
}
