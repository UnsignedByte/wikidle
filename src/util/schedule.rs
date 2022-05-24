use std::future::Future;
use std::thread::{
	self,
	JoinHandle
};
use tokio::{
	time::{Duration, interval},
	sync::oneshot,
	runtime::Builder,
};
// use tokio_core::reactor::Core;
use futures::{
	FutureExt,
	pin_mut,
	future::{
		select_all,
		ready
	}
};
use std::mem;

pub struct Schedule {
	handle: JoinHandle<()>,
	terminator: oneshot::Sender<()>
}

impl Schedule {
	pub fn new <F> (f: F, i: Duration) -> Schedule
		where F : Fn() -> () + Send + Sync + 'static
	{
		let (tx, rx) = oneshot::channel::<()>();

		let rx = rx
			.then(|_| ready( () ))
			.shared();

		Schedule {
			handle: thread::spawn(move || {
				let rt = Builder::new_current_thread()
					.enable_time()
					.build().unwrap();

				rt.block_on(async move {
		      let mut i = interval(i);

					loop {
						let t = i.tick()
							.then(|_| ready( () ));
						let rx = rx.clone();

						pin_mut!(t);
						pin_mut!(rx);

						let v: Vec<Box<dyn Future<Output = ()> + Unpin>>
							= vec![Box::new(t), Box::new(rx)];

						match select_all(v).await {
							(_, 0, _) => f(),
							(_, 1, _) => break,
							_ => panic!("Invalid pattern match")
						};

					};
				})
			}),
			terminator: tx
		}
	}

	pub fn terminate(self) {
		let _ = self.terminator.send( () );
	}
}