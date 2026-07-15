//! gRPC ingest worker library.

mod db;
mod grpc;
mod jobs;
mod load_job;

pub use grpc::IngestWorker;
