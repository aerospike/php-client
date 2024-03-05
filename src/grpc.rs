use tokio::runtime::{Builder, Runtime};

use std::convert::TryFrom;
use tokio::net::UnixStream;
use tonic::transport::{Endpoint, Uri};
use tower::service_fn;

#[path = "com.aerospike.daemon.rs"]
pub mod proto;

use crate::proto::kvs_client::KvsClient;

type StdError = Box<dyn std::error::Error + Send + Sync + 'static>;
type Result<T, E = StdError> = ::std::result::Result<T, E>;

// The order of the fields in this struct is important. They must be ordered
// such that when `BlockingClient` is dropped the client is dropped
// before the runtime. Not doing this will result in a deadlock when dropped.
// Rust drops struct fields in declaration order.
pub struct BlockingClient {
    client: KvsClient<tonic::transport::Channel>,
    rt: Runtime,
}

impl BlockingClient {
    // pub fn connect(path: String) -> Result<Self, tonic::transport::Error> {
    pub fn connect(path: String) -> Result<Self, tonic::transport::Error> {
        // let rt = Builder::new_multi_thread().enable_all().build().unwrap();
        let rt = Builder::new_current_thread().enable_all().build().unwrap();

        let binding = Endpoint::try_from("http://[::]:50051")?;
        let ch = binding.connect_with_connector(service_fn(move |_: Uri| {
            // Connect to a Uds socket
            UnixStream::connect(path.clone())
        }));

        // We will ignore this uri because uds do not use it
        // if your connector does use the uri it will be provided
        // as the request to the `MakeConnection`.
        let channel = rt.block_on(ch)?;

        let client = KvsClient::new(channel);

        Ok(Self { client, rt })
    }

    pub fn get(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeGetRequest>,
    ) -> Result<tonic::Response<proto::AerospikeSingleResponse>, tonic::Status> {
        self.rt.block_on(self.client.get(request))
    }

    pub fn get_header(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeGetHeaderRequest>,
    ) -> Result<tonic::Response<proto::AerospikeSingleResponse>, tonic::Status> {
        self.rt.block_on(self.client.get_header(request))
    }

    pub fn exists(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeExistsRequest>,
    ) -> Result<tonic::Response<proto::AerospikeExistsResponse>, tonic::Status> {
        self.rt.block_on(self.client.exists(request))
    }

    pub fn put(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikePutRequest>,
    ) -> Result<tonic::Response<proto::Error>, tonic::Status> {
        self.rt.block_on(self.client.put(request))
    }

    pub fn add(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikePutRequest>,
    ) -> Result<tonic::Response<proto::Error>, tonic::Status> {
        self.rt.block_on(self.client.add(request))
    }

    pub fn append(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikePutRequest>,
    ) -> Result<tonic::Response<proto::Error>, tonic::Status> {
        self.rt.block_on(self.client.append(request))
    }

    pub fn prepend(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikePutRequest>,
    ) -> Result<tonic::Response<proto::Error>, tonic::Status> {
        self.rt.block_on(self.client.prepend(request))
    }

    pub fn delete(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeDeleteRequest>,
    ) -> Result<tonic::Response<proto::AerospikeDeleteResponse>, tonic::Status> {
        self.rt.block_on(self.client.delete(request))
    }

    pub fn touch(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeTouchRequest>,
    ) -> Result<tonic::Response<proto::Error>, tonic::Status> {
        self.rt.block_on(self.client.touch(request))
    }

    pub fn batch_operate(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeBatchOperateRequest>,
    ) -> Result<tonic::Response<proto::AerospikeBatchOperateResponse>, tonic::Status> {
        self.rt.block_on(self.client.batch_operate(request))
    }

    pub fn create_index(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeCreateIndexRequest>,
    ) -> Result<tonic::Response<proto::AerospikeCreateIndexResponse>, tonic::Status> {
        self.rt.block_on(self.client.create_index(request))
    }

    pub fn drop_index(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeDropIndexRequest>,
    ) -> Result<tonic::Response<proto::AerospikeDropIndexResponse>, tonic::Status> {
        // return the task ID and handle the task
        self.rt.block_on(self.client.drop_index(request))
    }

    pub fn truncate(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeTruncateRequest>,
    ) -> Result<tonic::Response<proto::AerospikeTruncateResponse>, tonic::Status> {
        // return the task ID and handle the task
        self.rt.block_on(self.client.truncate(request))
    }

    pub fn register_udf(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeRegisterUdfRequest>,
    ) -> Result<tonic::Response<proto::AerospikeRegisterUdfResponse>, tonic::Status> {
        // return the task ID and handle the task
        self.rt.block_on(self.client.register_udf(request))
    }

    pub fn drop_udf(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeDropUdfRequest>,
    ) -> Result<tonic::Response<proto::AerospikeDropUdfResponse>, tonic::Status> {
        // return the task ID and handle the task
        self.rt.block_on(self.client.drop_udf(request))
    }

    pub fn list_udf(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeListUdfRequest>,
    ) -> Result<tonic::Response<proto::AerospikeListUdfResponse>, tonic::Status> {
        // return the task ID and handle the task
        self.rt.block_on(self.client.list_udf(request))
    }

    pub fn udf_execute(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeUdfExecuteRequest>,
    ) -> Result<tonic::Response<proto::AerospikeUdfExecuteResponse>, tonic::Status> {
        // return the task ID and handle the task
        self.rt.block_on(self.client.udf_execute(request))
    }
}
