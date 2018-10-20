use futures_cpupool;

lazy_static! {
	/// A CPU for processing independant, asynchronous operations
	pub static ref WORKER: futures_cpupool::CpuPool = futures_cpupool::Builder::new().pool_size(3).create();

	/// A work queue for tasks that need to be completed sequentially
	pub static ref QUEUED_WORKER: futures_cpupool::CpuPool = futures_cpupool::Builder::new().pool_size(1).create();
}
