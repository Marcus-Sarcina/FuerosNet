//! The client on a thread of its own.
//!
//! `Client` reaches its device through `Rc` (`rhtn-client`'s `Device`),
//! so it is built where it runs and never moves.  Everything else in the
//! process, the transport's tasks and the node's request handlers among
//! them, talks to it through a handle that carries a closure across.

use rhtn_archive::Keyhash;
use rhtn_client::ceremony::Client;
use std::sync::mpsc;

type Job = Box<dyn FnOnce(&mut Client) + Send>;

/// A client running on a thread of its own.
#[derive(Clone)]
pub struct Handle {
    tx: mpsc::Sender<Job>,
    me: Keyhash,
}

impl Handle {
    /// Build the client on a thread of its own: `build` runs there, and
    /// the client lives as long as a handle does.
    pub fn spawn(build: impl FnOnce() -> Client + Send + 'static) -> Handle {
        let (tx, rx) = mpsc::channel::<Job>();
        let (ktx, krx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut client = build();
            let _ = ktx.send(client.keyhash());
            while let Ok(job) = rx.recv() {
                job(&mut client);
            }
        });
        let me = krx.recv().expect("the client thread starts");
        Handle { tx, me }
    }

    pub fn me(&self) -> Keyhash {
        self.me
    }

    /// Run `f` on the client and take what it returns.
    pub async fn with<R, F>(&self, f: F) -> R
    where
        R: Send + 'static,
        F: FnOnce(&mut Client) -> R + Send + 'static,
    {
        let (rtx, rrx) = tokio::sync::oneshot::channel();
        self.tx
            .send(Box::new(move |c| {
                let _ = rtx.send(f(c));
            }))
            .expect("the client thread is running");
        rrx.await.expect("the client answers")
    }

    /// `with`, from outside a runtime.
    pub fn with_blocking<R, F>(&self, f: F) -> R
    where
        R: Send + 'static,
        F: FnOnce(&mut Client) -> R + Send + 'static,
    {
        let (rtx, rrx) = mpsc::channel();
        self.tx
            .send(Box::new(move |c| {
                let _ = rtx.send(f(c));
            }))
            .expect("the client thread is running");
        rrx.recv().expect("the client answers")
    }
}
