use tokio::runtime::{Builder, Runtime};

use std::convert::TryFrom;
use tokio::net::UnixStream;
use tonic::transport::{Endpoint, Uri};

use tokio_stream::{StreamExt};

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

        // set the maximum message size possible for a record: 128MiB for memory namespaces, with overhead
        let client = KvsClient::new(channel).max_decoding_message_size(130 * 1024 * 1024);

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
        self.rt.block_on(self.client.drop_index(request))
    }

    pub fn truncate(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeTruncateRequest>,
    ) -> Result<tonic::Response<proto::AerospikeTruncateResponse>, tonic::Status> {
        self.rt.block_on(self.client.truncate(request))
    }

    pub fn register_udf(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeRegisterUdfRequest>,
    ) -> Result<tonic::Response<proto::AerospikeRegisterUdfResponse>, tonic::Status> {
        self.rt.block_on(self.client.register_udf(request))
    }

    pub fn drop_udf(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeDropUdfRequest>,
    ) -> Result<tonic::Response<proto::AerospikeDropUdfResponse>, tonic::Status> {
        self.rt.block_on(self.client.drop_udf(request))
    }

    pub fn list_udf(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeListUdfRequest>,
    ) -> Result<tonic::Response<proto::AerospikeListUdfResponse>, tonic::Status> {
        self.rt.block_on(self.client.list_udf(request))
    }

    pub fn udf_execute(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeUdfExecuteRequest>,
    ) -> Result<tonic::Response<proto::AerospikeUdfExecuteResponse>, tonic::Status> {
        self.rt.block_on(self.client.udf_execute(request))
    }

    pub fn create_user(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeCreateUserRequest>,
    ) -> Result<tonic::Response<proto::AerospikeCreateUserResponse>, tonic::Status> {
        self.rt.block_on(self.client.create_user(request))
    }

    pub fn drop_user(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeDropUserRequest>,
    ) -> Result<tonic::Response<proto::AerospikeDropUserResponse>, tonic::Status> {
        self.rt.block_on(self.client.drop_user(request))
    }

    pub fn change_password(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeChangePasswordRequest>,
    ) -> Result<tonic::Response<proto::AerospikeChangePasswordResponse>, tonic::Status> {
        self.rt.block_on(self.client.change_password(request))
    }

    pub fn grant_roles(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeGrantRolesRequest>,
    ) -> Result<tonic::Response<proto::AerospikeGrantRolesResponse>, tonic::Status> {
        self.rt.block_on(self.client.grant_roles(request))
    }

    pub fn revoke_roles(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeRevokeRolesRequest>,
    ) -> Result<tonic::Response<proto::AerospikeRevokeRolesResponse>, tonic::Status> {
        self.rt.block_on(self.client.revoke_roles(request))
    }

    pub fn query_users(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeQueryUsersRequest>,
    ) -> Result<tonic::Response<proto::AerospikeQueryUsersResponse>, tonic::Status> {
        self.rt.block_on(self.client.query_users(request))
    }

    pub fn query_roles(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeQueryRolesRequest>,
    ) -> Result<tonic::Response<proto::AerospikeQueryRolesResponse>, tonic::Status> {
        self.rt.block_on(self.client.query_roles(request))
    }

    pub fn create_role(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeCreateRoleRequest>,
    ) -> Result<tonic::Response<proto::AerospikeCreateRoleResponse>, tonic::Status> {
        self.rt.block_on(self.client.create_role(request))
    }

    pub fn drop_role(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeDropRoleRequest>,
    ) -> Result<tonic::Response<proto::AerospikeDropRoleResponse>, tonic::Status> {
        self.rt.block_on(self.client.drop_role(request))
    }

    pub fn grant_privileges(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeGrantPrivilegesRequest>,
    ) -> Result<tonic::Response<proto::AerospikeGrantPrivilegesResponse>, tonic::Status> {
        self.rt.block_on(self.client.grant_privileges(request))
    }

    pub fn revoke_privileges(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeRevokePrivilegesRequest>,
    ) -> Result<tonic::Response<proto::AerospikeRevokePrivilegesResponse>, tonic::Status> {
        self.rt.block_on(self.client.revoke_privileges(request))
    }

    pub fn set_allowlist(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeSetAllowlistRequest>,
    ) -> Result<tonic::Response<proto::AerospikeSetAllowlistResponse>, tonic::Status> {
        self.rt.block_on(self.client.set_allowlist(request))
    }

    pub fn set_quotas(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeSetQuotasRequest>,
    ) -> Result<tonic::Response<proto::AerospikeSetQuotasResponse>, tonic::Status> {
        self.rt.block_on(self.client.set_quotas(request))
    }

    pub fn scan(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeScanRequest>,
    ) -> Result<tonic::Response<tonic::Streaming<proto::AerospikeStreamResponse>>, tonic::Status> {
        self.rt.block_on(self.client.scan(request))
    }

    pub fn query(
        &mut self,
        request: impl tonic::IntoRequest<proto::AerospikeQueryRequest>,
    ) -> Result<tonic::Response<tonic::Streaming<proto::AerospikeStreamResponse>>, tonic::Status> {
        self.rt.block_on(self.client.query(request))
    }

    pub fn next_record(
        &mut self,
        rs: &mut tonic::Streaming<proto::AerospikeStreamResponse>
    ) -> Option<Result<proto::AerospikeStreamResponse, tonic::Status>> {
        self.rt.block_on(rs.next())
    }

}
