package main

import (
	"context"
	"log/slog"
	"time"

	aero "github.com/aerospike/aerospike-client-go/v7"

	"github.com/aerospike/php-client/asld/proto"
	pb "github.com/aerospike/php-client/asld/proto"
)

const UNREACHABLE = "UNREACHABLE"

type server struct {
	pb.UnimplementedKVSServer

	client *aero.Client
	logger slog.Logger
}

func (s *server) Version(ctx context.Context, _ *pb.AerospikeVersionRequest) (*pb.AerospikeVersionResponse, error) {
	return &pb.AerospikeVersionResponse{
		Version: version,
	}, nil
}

func (s *server) Get(ctx context.Context, in *pb.AerospikeGetRequest) (*pb.AerospikeSingleResponse, error) {
	policy := toReadPolicy(in.Policy)
	key := toKey(in.Key)
	rec, err := s.client.Get(policy, key, in.BinNames...)
	if err != nil {
		return &pb.AerospikeSingleResponse{
			Error:  fromError(err),
			Record: fromRecord(rec),
		}, nil
	}

	return &pb.AerospikeSingleResponse{
		Record: fromRecord(rec),
	}, nil
}

func (s *server) GetHeader(ctx context.Context, in *pb.AerospikeGetHeaderRequest) (*pb.AerospikeSingleResponse, error) {
	policy := toReadPolicy(in.Policy)
	key := toKey(in.Key)
	rec, err := s.client.GetHeader(policy, key)
	if err != nil {
		return &pb.AerospikeSingleResponse{
			Error:  fromError(err),
			Record: fromRecord(rec),
		}, nil
	}

	return &pb.AerospikeSingleResponse{
		Record: fromRecord(rec),
	}, nil
}

func (s *server) Exists(ctx context.Context, in *pb.AerospikeExistsRequest) (*pb.AerospikeExistsResponse, error) {
	policy := toReadPolicy(in.Policy)
	key := toKey(in.Key)
	exists, err := s.client.Exists(policy, key)
	if err != nil {
		return &pb.AerospikeExistsResponse{
			Error:  fromError(err),
			Exists: &exists,
		}, nil
	}

	return &pb.AerospikeExistsResponse{
		Exists: &exists,
	}, nil
}

func (s *server) Put(ctx context.Context, in *pb.AerospikePutRequest) (*pb.Error, error) {
	policy := toWritePolicy(in.Policy)
	key := toKey(in.Key)
	err := s.client.PutBins(policy, key, toBins(in.Bins)...)
	if err != nil {
		return fromError(err), nil
	}

	return &pb.Error{
		ResultCode: 0,
	}, nil
}

func (s *server) Add(ctx context.Context, in *pb.AerospikePutRequest) (*pb.Error, error) {
	policy := toWritePolicy(in.Policy)
	key := toKey(in.Key)
	err := s.client.AddBins(policy, key, toBins(in.Bins)...)
	if err != nil {
		return fromError(err), nil
	}

	return &pb.Error{
		ResultCode: 0,
	}, nil
}

func (s *server) Append(ctx context.Context, in *pb.AerospikePutRequest) (*pb.Error, error) {
	policy := toWritePolicy(in.Policy)
	key := toKey(in.Key)
	err := s.client.AppendBins(policy, key, toBins(in.Bins)...)
	if err != nil {
		return fromError(err), nil
	}

	return &pb.Error{
		ResultCode: 0,
	}, nil
}

func (s *server) Prepend(ctx context.Context, in *pb.AerospikePutRequest) (*pb.Error, error) {
	policy := toWritePolicy(in.Policy)
	key := toKey(in.Key)
	err := s.client.PrependBins(policy, key, toBins(in.Bins)...)
	if err != nil {
		return fromError(err), nil
	}

	return &pb.Error{
		ResultCode: 0,
	}, nil
}

func (s *server) Delete(ctx context.Context, in *pb.AerospikeDeleteRequest) (*pb.AerospikeDeleteResponse, error) {
	policy := toWritePolicy(in.Policy)
	key := toKey(in.Key)
	existed, err := s.client.Delete(policy, key)
	if err != nil {
		return &pb.AerospikeDeleteResponse{
			Error:   fromError(err),
			Existed: &existed,
		}, nil
	}

	return &pb.AerospikeDeleteResponse{
		Existed: &existed,
	}, nil
}

func (s *server) Touch(ctx context.Context, in *pb.AerospikeTouchRequest) (*pb.Error, error) {
	policy := toWritePolicy(in.Policy)
	key := toKey(in.Key)
	err := s.client.Touch(policy, key)
	if err != nil {
		return fromError(err), nil
	}

	return &pb.Error{
		ResultCode: 0,
	}, nil
}

func (s *server) BatchOperate(ctx context.Context, in *pb.AerospikeBatchOperateRequest) (*pb.AerospikeBatchOperateResponse, error) {
	brecs := toBatchRecords(in.Records)
	err := s.client.BatchOperate(toBatchPolicy(in.Policy), brecs)
	if err != nil {
		return &pb.AerospikeBatchOperateResponse{
			Error:   fromError(err),
			Records: fromBatchRecords(brecs),
		}, nil
	}

	return &pb.AerospikeBatchOperateResponse{
		Records: fromBatchRecords(brecs),
	}, nil
}

func (s *server) CreateIndex(ctx context.Context, in *pb.AerospikeCreateIndexRequest) (*pb.AerospikeCreateIndexResponse, error) {
	// TODO(Khosrow): return the task
	_, err := s.client.CreateComplexIndex(toWritePolicy(in.Policy), in.Namespace, in.SetName, in.IndexName, in.BinName, toIndexType(in.IndexType), toIndexCollectionType(in.IndexCollectionType))
	if err != nil {
		return &pb.AerospikeCreateIndexResponse{
			Error: fromError(err),
		}, nil
	}

	return &pb.AerospikeCreateIndexResponse{Error: nil}, nil
}

func (s *server) DropIndex(ctx context.Context, in *pb.AerospikeDropIndexRequest) (*pb.AerospikeDropIndexResponse, error) {
	err := s.client.DropIndex(toWritePolicy(in.Policy), in.Namespace, in.SetName, in.IndexName)
	if err != nil {
		return &pb.AerospikeDropIndexResponse{
			Error: fromError(err),
		}, nil
	}

	return &pb.AerospikeDropIndexResponse{Error: nil}, nil
}

func (s *server) Truncate(ctx context.Context, in *pb.AerospikeTruncateRequest) (*pb.AerospikeTruncateResponse, error) {
	err := s.client.Truncate(toInfoPolicy(in.Policy), in.Namespace, in.SetName, toTime(in.BeforeNanos))
	if err != nil {
		return &pb.AerospikeTruncateResponse{
			Error: fromError(err),
		}, nil
	}

	return &pb.AerospikeTruncateResponse{Error: nil}, nil
}

func (s *server) RegisterUDF(ctx context.Context, in *pb.AerospikeRegisterUDFRequest) (*pb.AerospikeRegisterUDFResponse, error) {
	_, err := s.client.RegisterUDF(toWritePolicy(in.Policy), in.UdfBody, in.PackageName, toLanguage(in.Language))
	if err != nil {
		return &pb.AerospikeRegisterUDFResponse{
			Error: fromError(err),
		}, nil
	}

	return &pb.AerospikeRegisterUDFResponse{Error: nil}, nil
}

func (s *server) DropUDF(ctx context.Context, in *pb.AerospikeDropUDFRequest) (*pb.AerospikeDropUDFResponse, error) {
	_, err := s.client.RemoveUDF(toWritePolicy(in.Policy), in.PackageName)
	if err != nil {
		return &pb.AerospikeDropUDFResponse{
			Error: fromError(err),
		}, nil
	}

	return &pb.AerospikeDropUDFResponse{Error: nil}, nil
}

func (s *server) ListUDF(ctx context.Context, in *pb.AerospikeListUDFRequest) (*pb.AerospikeListUDFResponse, error) {
	udfList, err := s.client.ListUDF(toReadPolicy(in.Policy))
	if err != nil {
		return &pb.AerospikeListUDFResponse{
			Error: fromError(err),
		}, nil
	}

	return &pb.AerospikeListUDFResponse{Error: nil, UdfList: fromUDFs(udfList)}, nil
}

func (s *server) UDFExecute(ctx context.Context, in *pb.AerospikeUDFExecuteRequest) (*pb.AerospikeUDFExecuteResponse, error) {
	res, err := s.client.Execute(toWritePolicy(in.Policy), toKey(in.Key), in.PackageName, in.FunctionName, toValues(in.Args)...)
	if err != nil {
		return &pb.AerospikeUDFExecuteResponse{
			Error: fromError(err),
		}, nil
	}

	return &pb.AerospikeUDFExecuteResponse{Error: nil, Result: fromValue(res)}, nil
}

func (s *server) CreateUser(ctx context.Context, in *pb.AerospikeCreateUserRequest) (*pb.AerospikeCreateUserResponse, error) {
	err := s.client.CreateUser(toAdminPolicy(in.Policy), in.User, in.Password, in.Roles)
	return &pb.AerospikeCreateUserResponse{
		Error: fromError(err),
	}, nil
}

func (s *server) DropUser(ctx context.Context, in *pb.AerospikeDropUserRequest) (*pb.AerospikeDropUserResponse, error) {
	err := s.client.DropUser(toAdminPolicy(in.Policy), in.User)
	return &pb.AerospikeDropUserResponse{
		Error: fromError(err),
	}, nil
}

func (s *server) ChangePassword(ctx context.Context, in *pb.AerospikeChangePasswordRequest) (*pb.AerospikeChangePasswordResponse, error) {
	err := s.client.ChangePassword(toAdminPolicy(in.Policy), in.User, in.Password)
	return &pb.AerospikeChangePasswordResponse{
		Error: fromError(err),
	}, nil
}

func (s *server) GrantRoles(ctx context.Context, in *pb.AerospikeGrantRolesRequest) (*pb.AerospikeGrantRolesResponse, error) {
	err := s.client.GrantRoles(toAdminPolicy(in.Policy), in.User, in.Roles)
	return &pb.AerospikeGrantRolesResponse{
		Error: fromError(err),
	}, nil
}

func (s *server) RevokeRoles(ctx context.Context, in *pb.AerospikeRevokeRolesRequest) (*pb.AerospikeRevokeRolesResponse, error) {
	err := s.client.RevokeRoles(toAdminPolicy(in.Policy), in.User, in.Roles)
	return &pb.AerospikeRevokeRolesResponse{
		Error: fromError(err),
	}, nil
}

func (s *server) QueryUsers(ctx context.Context, in *pb.AerospikeQueryUsersRequest) (*pb.AerospikeQueryUsersResponse, error) {
	var err aero.Error
	var res []*aero.UserRoles
	if in.User != nil {
		u, err1 := s.client.QueryUser(toAdminPolicy(in.Policy), *in.User)
		res = []*aero.UserRoles{u}
		err = err1
	} else {
		res, err = s.client.QueryUsers(toAdminPolicy(in.Policy))
	}

	if err != nil {
		return &pb.AerospikeQueryUsersResponse{
			Error: fromError(err),
		}, nil
	}

	return &pb.AerospikeQueryUsersResponse{Error: nil, UserRoles: fromUserRoles(res)}, nil
}

func (s *server) QueryRoles(ctx context.Context, in *pb.AerospikeQueryRolesRequest) (*pb.AerospikeQueryRolesResponse, error) {
	var err aero.Error
	var res []*aero.Role
	if in.RoleName != nil {
		u, err1 := s.client.QueryRole(toAdminPolicy(in.Policy), *in.RoleName)
		res = []*aero.Role{u}
		err = err1
	} else {
		res, err = s.client.QueryRoles(toAdminPolicy(in.Policy))
	}

	if err != nil {
		return &pb.AerospikeQueryRolesResponse{
			Error: fromError(err),
		}, nil
	}

	return &pb.AerospikeQueryRolesResponse{Error: nil, Roles: fromRoles(res)}, nil
}

func (s *server) CreateRole(ctx context.Context, in *pb.AerospikeCreateRoleRequest) (*pb.AerospikeCreateRoleResponse, error) {
	err := s.client.CreateRole(toAdminPolicy(in.Policy), in.RoleName, toPrivileges(in.Privileges), in.Allowlist, in.ReadQuota, in.WriteQuota)
	return &pb.AerospikeCreateRoleResponse{
		Error: fromError(err),
	}, nil
}

func (s *server) DropRole(ctx context.Context, in *pb.AerospikeDropRoleRequest) (*pb.AerospikeDropRoleResponse, error) {
	err := s.client.DropRole(toAdminPolicy(in.Policy), in.RoleName)
	return &pb.AerospikeDropRoleResponse{
		Error: fromError(err),
	}, nil
}

func (s *server) GrantPrivileges(ctx context.Context, in *pb.AerospikeGrantPrivilegesRequest) (*pb.AerospikeGrantPrivilegesResponse, error) {
	err := s.client.GrantPrivileges(toAdminPolicy(in.Policy), in.RoleName, toPrivileges(in.Privileges))
	return &pb.AerospikeGrantPrivilegesResponse{
		Error: fromError(err),
	}, nil
}

func (s *server) RevokePrivileges(ctx context.Context, in *pb.AerospikeRevokePrivilegesRequest) (*pb.AerospikeRevokePrivilegesResponse, error) {
	err := s.client.RevokePrivileges(toAdminPolicy(in.Policy), in.RoleName, toPrivileges(in.Privileges))
	return &pb.AerospikeRevokePrivilegesResponse{
		Error: fromError(err),
	}, nil
}

func (s *server) SetAllowlist(ctx context.Context, in *pb.AerospikeSetAllowlistRequest) (*pb.AerospikeSetAllowlistResponse, error) {
	err := s.client.SetWhitelist(toAdminPolicy(in.Policy), in.RoleName, in.Allowlist)
	return &pb.AerospikeSetAllowlistResponse{
		Error: fromError(err),
	}, nil
}

func (s *server) SetQuotas(ctx context.Context, in *pb.AerospikeSetQuotasRequest) (*pb.AerospikeSetQuotasResponse, error) {
	err := s.client.SetQuotas(toAdminPolicy(in.Policy), in.RoleName, in.ReadQuota, in.WriteQuota)
	return &pb.AerospikeSetQuotasResponse{
		Error: fromError(err),
	}, nil
}

func (s *server) Scan(in *pb.AerospikeScanRequest, stream pb.KVS_ScanServer) error {
	rs, err := s.client.ScanPartitions(toScanPolicy(in.Policy), toPartitionFilter(in.PartitionFilter), in.Namespace, in.SetName, in.BinNames...)
	if err != nil {
		stream.Send(&pb.AerospikeStreamResponse{
			Error: fromError(err),
		})
		return err
	}

	for res := range rs.Results() {
		if res.Err != nil {
			if err := stream.Send(&pb.AerospikeStreamResponse{Error: fromError(res.Err)}); err != nil {
				rs.Close()
				return err
			}
			continue
		}

		// send the record
		resp := pb.AerospikeStreamResponse{Record: fromRecord(res.Record)}
		resp.Bval = res.BVal

		if err := stream.Send(&resp); err != nil {
			rs.Close()
			return err
		}
	}

	return nil
}

func (s *server) Query(in *pb.AerospikeQueryRequest, stream pb.KVS_QueryServer) error {
	rs, err := s.client.QueryPartitions(toQueryPolicy(in.Policy), toStatement(in.Statement), toPartitionFilter(in.PartitionFilter))
	if err != nil {
		stream.Send(&pb.AerospikeStreamResponse{
			Error: fromError(err),
		})
		return err
	}

	for res := range rs.Results() {
		if res.Err != nil {
			if err := stream.Send(&pb.AerospikeStreamResponse{Error: fromError(res.Err)}); err != nil {
				rs.Close()
				return err
			}
			continue
		}

		// send the record
		resp := pb.AerospikeStreamResponse{Record: fromRecord(res.Record)}
		resp.Bval = res.BVal

		if err := stream.Send(&resp); err != nil {
			rs.Close()
			return err
		}
	}

	return nil
}

func toStatement(in *pb.Statement) *aero.Statement {
	if in != nil {
		var idxName string
		if in.IndexName != nil {
			idxName = *in.IndexName
		}
		return &aero.Statement{
			Namespace: in.Namespace,
			SetName:   in.SetName,
			IndexName: idxName,
			BinNames:  in.BinNames,
			Filter:    toFilter(in.Filter),
			// packageName:,
			// functionName:,
			// functionArgs:,
			TaskId:     in.TaskId,
			ReturnData: in.ReturnData,
		}
	}
	return nil
}

func toCDTContexts(in []*pb.CDTContext) []*aero.CDTContext {
	res := make([]*aero.CDTContext, len(in))
	for i := range in {
		res[i] = toCDTContext(in[i])
	}
	return res
}

func toCDTContext(in *pb.CDTContext) *aero.CDTContext {
	if in != nil {
		return &aero.CDTContext{Id: int(in.Id), Value: toValue(in.Value)}
	}
	return nil
}

func toFilter(in *pb.QueryFilter) *aero.Filter {
	if in != nil {
		return aero.NewFilter(
			in.Name,
			toIndexCollectionType(in.IdxType),
			int(in.ValueParticleType),
			toValue(in.Begin),
			toValue(in.End),
			toCDTContexts(in.Ctx),
		)
	}
	return nil
}

func toPartitionFilter(in *pb.PartitionFilter) *aero.PartitionFilter {
	if in != nil {
		return &aero.PartitionFilter{
			Begin:      int(in.Begin),
			Count:      int(in.Count),
			Digest:     in.Digest,
			Partitions: toPartitionStatuses(in.Partitions),
			Done:       in.Done,
			Retry:      in.Retry,
		}
	}
	return nil
}

func toPartitionStatuses(in []*pb.PartitionStatus) []*aero.PartitionStatus {
	res := make([]*aero.PartitionStatus, len(in))
	for i := range in {
		res[i] = toPartitionStatus(in[i])
	}
	return res
}

func toPartitionStatus(in *pb.PartitionStatus) *aero.PartitionStatus {
	var bval int64 = 0
	if in != nil {
		bval = 0
		if in.Bval != nil {
			bval = *in.Bval
		}
		return &aero.PartitionStatus{
			BVal:   bval,
			Id:     int(in.Id),
			Retry:  in.Retry,
			Digest: in.Digest,
		}
	}
	return nil
}

func toPrivileges(in []*proto.Privilege) []aero.Privilege {
	res := make([]aero.Privilege, len(in))
	for i := range in {
		res[i] = toPrivilege(in[i])
	}
	return res
}

func toPrivilege(in *proto.Privilege) aero.Privilege {
	switch in.Name {
	case string(aero.UserAdmin):
		return aero.Privilege{Code: aero.UserAdmin, Namespace: in.Namespace, SetName: in.SetName}
	case string(aero.SysAdmin):
		return aero.Privilege{Code: aero.SysAdmin, Namespace: in.Namespace, SetName: in.SetName}
	case string(aero.DataAdmin):
		return aero.Privilege{Code: aero.DataAdmin, Namespace: in.Namespace, SetName: in.SetName}
	case string(aero.UDFAdmin):
		return aero.Privilege{Code: aero.UDFAdmin, Namespace: in.Namespace, SetName: in.SetName}
	case string(aero.SIndexAdmin):
		return aero.Privilege{Code: aero.SIndexAdmin, Namespace: in.Namespace, SetName: in.SetName}
	case string(aero.ReadWriteUDF):
		return aero.Privilege{Code: aero.ReadWriteUDF, Namespace: in.Namespace, SetName: in.SetName}
	case string(aero.ReadWrite):
		return aero.Privilege{Code: aero.ReadWrite, Namespace: in.Namespace, SetName: in.SetName}
	case string(aero.Read):
		return aero.Privilege{Code: aero.Read, Namespace: in.Namespace, SetName: in.SetName}
	case string(aero.Write):
		return aero.Privilege{Code: aero.Write, Namespace: in.Namespace, SetName: in.SetName}
	case string(aero.Truncate):
		return aero.Privilege{Code: aero.Truncate, Namespace: in.Namespace, SetName: in.SetName}
	}
	panic("UNREACHABLE")
}

func fromPrivileges(in []aero.Privilege) []*proto.Privilege {
	res := make([]*proto.Privilege, len(in))
	for i := range in {
		res[i] = fromPrivilege(&in[i])
	}
	return res
}

func fromPrivilege(in *aero.Privilege) *proto.Privilege {
	return &proto.Privilege{
		Name:      string(in.Code),
		Namespace: in.Namespace,
		SetName:   in.SetName,
	}
}

func fromRoles(in []*aero.Role) []*proto.Role {
	res := make([]*proto.Role, len(in))
	for i := range in {
		res[i] = fromRole(in[i])
	}
	return res
}

func fromRole(in *aero.Role) *proto.Role {
	return &proto.Role{
		Name:       in.Name,
		Privileges: fromPrivileges(in.Privileges),
		Allowlist:  in.Whitelist,
		ReadQuota:  in.ReadQuota,
		WriteQuota: in.WriteQuota,
	}
}

type Int interface {
	int | uint | int64 | int32 | int16 | int8 | uint64 | uint32 | uint16 | uint8
}

func convIntList[T Int, U Int](l []T) []U {
	res := make([]U, len(l))
	for i := range l {
		res[i] = U(l[i])
	}
	return res
}

func fromUserRoles(in []*aero.UserRoles) []*proto.UserRole {
	res := make([]*proto.UserRole, len(in))
	for i := range in {
		res[i] = fromUserRole(in[i])
	}
	return res
}

func fromUserRole(in *aero.UserRoles) *proto.UserRole {
	return &proto.UserRole{
		User:       in.User,
		Roles:      in.Roles,
		ReadInfo:   convIntList[int, uint64](in.ReadInfo),
		WriteInfo:  convIntList[int, uint64](in.WriteInfo),
		ConnsInUse: uint64(in.ConnsInUse),
	}
}

func fromUDFs(in []*aero.UDF) []*proto.UDFMeta {
	res := make([]*proto.UDFMeta, len(in))
	for i := range in {
		res[i] = fromUDF(in[i])
	}
	return res
}

func fromUDF(in *aero.UDF) *proto.UDFMeta {
	return &proto.UDFMeta{
		PackageName: in.Filename,
		Hash:        in.Hash,
		Language:    fromLanguage(in.Language),
	}
}

func fromLanguage(in aero.Language) proto.UDFLanguage {
	switch in {
	case aero.LUA:
		return proto.UDFLanguage_LUA
	}
	panic(UNREACHABLE)
}

func toLanguage(in proto.UDFLanguage) aero.Language {
	switch in {
	case proto.UDFLanguage_LUA:
		return aero.LUA
	}
	panic(UNREACHABLE)
}

func toTime(in *int64) *time.Time {
	if in != nil {
		t := time.Unix(0, *in)
		return &t
	}
	return nil
}

func toIndexType(in pb.IndexType) aero.IndexType {
	switch in {
	case pb.IndexType_IndexTypeNumeric:
		return aero.NUMERIC
	case pb.IndexType_IndexTypeString:
		return aero.STRING
	case pb.IndexType_IndexTypeBlob:
		return aero.BLOB
	case pb.IndexType_IndexTypeGeo2DSphere:
		return aero.GEO2DSPHERE
	}

	panic(UNREACHABLE)
}

func toIndexCollectionType(in pb.IndexCollectionType) aero.IndexCollectionType {
	switch in {
	case pb.IndexCollectionType_IndexCollectionTypeDefault:
		return aero.ICT_DEFAULT
	case pb.IndexCollectionType_IndexCollectionTypeList:
		return aero.ICT_LIST
	case pb.IndexCollectionType_IndexCollectionTypeMapKeys:
		return aero.ICT_MAPKEYS
	case pb.IndexCollectionType_IndexCollectionTypeMapValues:
		return aero.ICT_MAPVALUES
	}

	panic(UNREACHABLE)
}

func toReadPolicy(in *pb.ReadPolicy) *aero.BasePolicy {
	if in != nil {
		return &aero.BasePolicy{
			FilterExpression:                  toExpression(in.FilterExpression),
			ReadModeAP:                        aero.ReadModeAP(in.ReadModeAP),
			ReadModeSC:                        aero.ReadModeSC(in.ReadModeSC),
			TotalTimeout:                      time.Duration(in.TotalTimeout * uint64(time.Millisecond)),
			SocketTimeout:                     time.Duration(in.SocketTimeout * uint64(time.Millisecond)),
			MaxRetries:                        int(in.MaxRetries),
			SleepBetweenRetries:               time.Duration(time.Duration(in.SleepBetweenRetries) * time.Millisecond),
			SleepMultiplier:                   in.SleepMultiplier,
			ExitFastOnExhaustedConnectionPool: in.ExitFastOnExhaustedConnectionPool,
			SendKey:                           in.SendKey,
			UseCompression:                    in.UseCompression,
			ReplicaPolicy:                     aero.ReplicaPolicy(in.ReplicaPolicy),
		}
	}
	return nil
}

func toWritePolicy(in *pb.WritePolicy) *aero.WritePolicy {
	if in != nil {
		return &aero.WritePolicy{
			BasePolicy:         *toReadPolicy(in.Policy),
			RecordExistsAction: aero.RecordExistsAction(in.RecordExistsAction),
			GenerationPolicy:   aero.GenerationPolicy(in.GenerationPolicy),
			CommitLevel:        aero.CommitLevel(in.CommitLevel),
			Generation:         in.Generation,
			Expiration:         in.Expiration,
			RespondPerEachOp:   in.RespondPerEachOp,
			DurableDelete:      in.DurableDelete,
		}
	}
	return nil
}

func toMultiPolicy(in *pb.MultiPolicy) *aero.MultiPolicy {
	if in != nil {
		return &aero.MultiPolicy{
			BasePolicy:         *toReadPolicy(in.ReadPolicy),
			MaxConcurrentNodes: int(in.MaxConcurrentNodes),

			MaxRecords:       int64(in.MaxRecords),
			RecordsPerSecond: int(in.RecordsPerSecond),
			RecordQueueSize:  int(in.RecordQueueSize),
			IncludeBinData:   in.IncludeBinData,
		}
	}
	return nil
}

func toScanPolicy(in *pb.ScanPolicy) *aero.ScanPolicy {
	if in != nil && in.MultiPolicy != nil {
		return &aero.ScanPolicy{
			MultiPolicy: *toMultiPolicy(in.MultiPolicy),
		}
	}
	return nil
}

func toQueryPolicy(in *pb.QueryPolicy) *aero.QueryPolicy {
	if in != nil && in.MultiPolicy != nil {
		return &aero.QueryPolicy{
			MultiPolicy:      *toMultiPolicy(in.MultiPolicy),
			ExpectedDuration: aero.QueryDuration(in.ExpectedDuration),
		}
	}
	return nil
}

func toAdminPolicy(in *pb.AdminPolicy) *aero.AdminPolicy {
	if in != nil {
		return &aero.AdminPolicy{Timeout: time.Duration(in.Timeout * uint32(time.Millisecond))}
	}
	return nil
}

func toInfoPolicy(in *pb.InfoPolicy) *aero.InfoPolicy {
	if in != nil {
		return &aero.InfoPolicy{Timeout: time.Duration(in.Timeout * uint32(time.Millisecond))}
	}
	return nil
}

func toBatchPolicy(in *pb.BatchPolicy) *aero.BatchPolicy {
	if in != nil {
		return &aero.BatchPolicy{
			BasePolicy:          *toReadPolicy(in.Policy),
			ConcurrentNodes:     int(*in.ConcurrentNodes),
			AllowInline:         in.AllowInline,
			AllowInlineSSD:      in.AllowInlineSSD,
			RespondAllKeys:      in.RespondAllKeys,
			AllowPartialResults: in.AllowPartialResults,
		}
	}
	return nil
}

func toBatchReadPolicy(in *pb.BatchReadPolicy) *aero.BatchReadPolicy {
	if in != nil {
		return &aero.BatchReadPolicy{
			FilterExpression: toExpression(in.FilterExpression),
			ReadModeAP:       aero.ReadModeAP(in.ReadModeAP),
			ReadModeSC:       aero.ReadModeSC(in.ReadModeSC),
		}
	}
	return nil
}

func toBatchWritePolicy(in *pb.BatchWritePolicy) *aero.BatchWritePolicy {
	if in != nil {
		return &aero.BatchWritePolicy{
			FilterExpression:   toExpression(in.FilterExpression),
			RecordExistsAction: aero.RecordExistsAction(in.RecordExistsAction),
			CommitLevel:        aero.CommitLevel(in.CommitLevel),
			GenerationPolicy:   aero.GenerationPolicy(in.GenerationPolicy),
			Generation:         in.Generation,
			Expiration:         in.Expiration,
			DurableDelete:      in.DurableDelete,
			SendKey:            in.SendKey,
		}
	}
	return nil
}

func toBatchDeletePolicy(in *pb.BatchDeletePolicy) *aero.BatchDeletePolicy {
	if in != nil {
		return &aero.BatchDeletePolicy{
			FilterExpression: toExpression(in.FilterExpression),
			CommitLevel:      aero.CommitLevel(in.CommitLevel),
			GenerationPolicy: aero.GenerationPolicy(in.GenerationPolicy),
			Generation:       in.Generation,
			DurableDelete:    in.DurableDelete,
			SendKey:          in.SendKey,
		}
	}
	return nil
}

func toBatchUDFPolicy(in *pb.BatchUDFPolicy) *aero.BatchUDFPolicy {
	if in != nil {
		return &aero.BatchUDFPolicy{
			FilterExpression: toExpression(in.FilterExpression),
			CommitLevel:      aero.CommitLevel(in.CommitLevel),
			Expiration:       in.Expiration,
			DurableDelete:    in.DurableDelete,
			SendKey:          in.SendKey,
		}
	}
	return nil
}

func toBatchRecords(in []*pb.BatchOperate) (res []aero.BatchRecordIfc) {
	for i := range in {
		res = append(res, toBatchRecordIfc(in[i]))
	}
	return res
}

func toBatchRecordIfc(in *pb.BatchOperate) aero.BatchRecordIfc {
	if in.Br != nil {
		if len(in.Br.BinNames) > 0 {
			return aero.NewBatchRead(toBatchReadPolicy(in.Br.Policy), toKey(in.Br.BatchRecord.Key), in.Br.BinNames)
		} else if len(in.Br.Ops) > 0 {
			return aero.NewBatchReadOps(toBatchReadPolicy(in.Br.Policy), toKey(in.Br.BatchRecord.Key), toOps(in.Br.Ops)...)
		} else if in.Br.ReadAllBins {
			return aero.NewBatchRead(toBatchReadPolicy(in.Br.Policy), toKey(in.Br.BatchRecord.Key), nil)
		}
		return aero.NewBatchReadHeader(toBatchReadPolicy(in.Br.Policy), toKey(in.Br.BatchRecord.Key))
	} else if in.Bw != nil {
		return aero.NewBatchWrite(toBatchWritePolicy(in.Bw.Policy), toKey(in.Bw.BatchRecord.Key), toOps(in.Bw.Ops)...)
	} else if in.Bd != nil {
		return aero.NewBatchDelete(toBatchDeletePolicy(in.Bd.Policy), toKey(in.Bd.BatchRecord.Key))
	} else if in.Bu != nil {
		return aero.NewBatchUDF(toBatchUDFPolicy(in.Bu.Policy), toKey(in.Bu.BatchRecord.Key), in.Bu.PackageName, in.Bu.FunctionName, toValues(in.Bu.FunctionArgs)...)
	}

	panic(UNREACHABLE)
}

func toOps(in []*pb.Operation) (res []*aero.Operation) {
	for i := range in {
		res = append(res, toOp(in[i]))
	}

	return res
}

func toStdOp(in *pb.StdOperation) *aero.Operation {
	if in != nil {
		switch in.OpType {
		case pb.OperationType_OperationTypeGet:
			if in.BinName != nil {
				return aero.GetBinOp(*in.BinName)
			}
			return aero.GetOp()
		case pb.OperationType_OperationTypeGetHeader:
			return aero.GetHeaderOp()
		case pb.OperationType_OperationTypePut:
			return aero.PutOp(aero.NewBin(*in.BinName, toValue(in.BinValue)))
		case pb.OperationType_OperationTypeAdd:
			return aero.AddOp(aero.NewBin(*in.BinName, toValue(in.BinValue)))
		case pb.OperationType_OperationTypeAppend:
			return aero.AppendOp(aero.NewBin(*in.BinName, toValue(in.BinValue)))
		case pb.OperationType_OperationTypePrepend:
			return aero.PrependOp(aero.NewBin(*in.BinName, toValue(in.BinValue)))
		case pb.OperationType_OperationTypeTouch:
			return aero.TouchOp()
		case pb.OperationType_OperationTypeDelete:
			return aero.DeleteOp()
		}
	}

	panic(UNREACHABLE)
}

func toMapOrderType(in int64) aero.MapOrderTypes {
	switch in {
	case int64(pb.MapOrderType_MapOrderTypeUnordered):
		return aero.MapOrder.UNORDERED
	case int64(pb.MapOrderType_MapOrderTypeKeyOrdered):
		return aero.MapOrder.KEY_ORDERED
	case int64(pb.MapOrderType_MapOrderTypeKeyValueOrdered):
		return aero.MapOrder.KEY_VALUE_ORDERED
	}
	panic(UNREACHABLE)
}

func toCdtMapPolicy(in *pb.CdtMapPolicy) *aero.MapPolicy {
	if in != nil {
		if in.PersistedIndex {
			return aero.NewMapPolicyWithFlagsAndPersistedIndex(toMapOrderType(int64(in.GetMapOrder())), int(in.GetFlags()))
		}
		return aero.NewMapPolicyWithFlags(toMapOrderType(int64(in.GetMapOrder())), int(in.GetFlags()))
	}

	return aero.DefaultMapPolicy()
}

func toCdtMapReturnType(in *pb.CdtMapReturnType) aero.MapReturnTypes {
	if in != nil {
		switch *in {
		case pb.CdtMapReturnType_CdtMapReturnTypeNone:
			return aero.MapReturnType.NONE
		case pb.CdtMapReturnType_CdtMapReturnTypeIndex:
			return aero.MapReturnType.INDEX
		case pb.CdtMapReturnType_CdtMapReturnTypeReverseIndex:
			return aero.MapReturnType.REVERSE_INDEX
		case pb.CdtMapReturnType_CdtMapReturnTypeRank:
			return aero.MapReturnType.RANK
		case pb.CdtMapReturnType_CdtMapReturnTypeReverseRank:
			return aero.MapReturnType.REVERSE_RANK
		case pb.CdtMapReturnType_CdtMapReturnTypeCount:
			return aero.MapReturnType.COUNT
		case pb.CdtMapReturnType_CdtMapReturnTypeKey:
			return aero.MapReturnType.KEY
		case pb.CdtMapReturnType_CdtMapReturnTypeValue:
			return aero.MapReturnType.VALUE
		case pb.CdtMapReturnType_CdtMapReturnTypeKeyValue:
			return aero.MapReturnType.KEY_VALUE
		case pb.CdtMapReturnType_CdtMapReturnTypeExists:
			return aero.MapReturnType.EXISTS
		case pb.CdtMapReturnType_CdtMapReturnTypeUnorderedMap:
			return aero.MapReturnType.UNORDERED_MAP
		case pb.CdtMapReturnType_CdtMapReturnTypeOrderedMap:
			return aero.MapReturnType.ORDERED_MAP
		case pb.CdtMapReturnType_CdtMapReturnTypeInverted:
			return aero.MapReturnType.INVERTED
		}
	}

	return aero.MapReturnType.NONE
}

func toCdtMapOp(in *pb.CdtMapOperation) *aero.Operation {
	if in != nil {
		switch in.Op {
		case pb.CdtMapCommandOp_CdtMapCommandOpCreate:
			mapOrderType := toMapOrderType(in.Args[0].GetI())
			return aero.MapCreateOp(in.BinName, mapOrderType, toCDTContexts(in.Ctx))
		case pb.CdtMapCommandOp_CdtMapCommandOpSetPolicy:
			return aero.MapSetPolicyOp(toCdtMapPolicy(in.Policy), in.BinName, toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpPutItems:
			// implement ordered maps
			m := map[interface{}]interface{}(toValue(in.Args[0]).(aero.MapValue))
			return aero.MapPutItemsOp(toCdtMapPolicy(in.Policy), in.BinName, m, toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpIncrement:
			key := toValue(in.Args[0])
			incr := toValue(in.Args[1])
			return aero.MapIncrementOp(toCdtMapPolicy(in.Policy), in.BinName, key, incr, toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpDecrement:
			key := toValue(in.Args[0])
			decr := toValue(in.Args[1])
			return aero.MapDecrementOp(toCdtMapPolicy(in.Policy), in.BinName, key, decr, toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpClear:
			return aero.MapClearOp(in.BinName, toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpRemoveByKeyList:
			keys := toListOfValueIfcs(in.Args[0])
			if len(keys) == 1 {
				return aero.MapRemoveByKeyOp(in.BinName, keys[0], toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
			}
			return aero.MapRemoveByKeyListOp(in.BinName, keys, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpRemoveByKeyRange:
			begin := toValue(in.Args[0])
			end := toValue(in.Args[1])
			return aero.MapRemoveByKeyRangeOp(in.BinName, begin, end, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpRemoveByValueList:
			values := toListOfValueIfcs(in.Args[0])
			if len(values) == 1 {
				return aero.MapRemoveByValueOp(in.BinName, values[0], toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
			}
			return aero.MapRemoveByValueListOp(in.BinName, values, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpRemoveByValueRange:
			begin := toValue(in.Args[0])
			end := toValue(in.Args[1])
			return aero.MapRemoveByValueRangeOp(in.BinName, begin, end, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpRemoveByValueRelativeRankRange:
			value := toValue(in.Args[0])
			rank := int(in.Args[1].GetI())
			return aero.MapRemoveByValueRelativeRankRangeOp(in.BinName, value, rank, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpRemoveByValueRelativeRankRangeCount:
			value := toValue(in.Args[0])
			rank := int(in.Args[1].GetI())
			count := int(in.Args[2].GetI())
			return aero.MapRemoveByValueRelativeRankRangeCountOp(in.BinName, value, rank, count, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpRemoveByIndex:
			index := int(in.Args[0].GetI())
			return aero.MapRemoveByIndexOp(in.BinName, index, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpRemoveByIndexRange:
			index := int(in.Args[0].GetI())
			return aero.MapRemoveByIndexRangeOp(in.BinName, index, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpRemoveByIndexRangeCount:
			index := int(in.Args[0].GetI())
			count := int(in.Args[1].GetI())
			return aero.MapRemoveByIndexRangeCountOp(in.BinName, index, count, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpRemoveByRank:
			rank := int(in.Args[0].GetI())
			return aero.MapRemoveByRankOp(in.BinName, rank, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpRemoveByRankRange:
			rank := int(in.Args[0].GetI())
			return aero.MapRemoveByRankRangeOp(in.BinName, rank, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpRemoveByRankRangeCount:
			rank := int(in.Args[0].GetI())
			count := int(in.Args[1].GetI())
			return aero.MapRemoveByRankRangeCountOp(in.BinName, rank, count, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpRemoveByKeyRelativeIndexRange:
			key := toValue(in.Args[0])
			index := int(in.Args[1].GetI())
			return aero.MapRemoveByKeyRelativeIndexRangeOp(in.BinName, key, index, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpRemoveByKeyRelativeIndexRangeCount:
			key := toValue(in.Args[0])
			index := int(in.Args[1].GetI())
			count := int(in.Args[2].GetI())
			return aero.MapRemoveByKeyRelativeIndexRangeCountOp(in.BinName, key, index, count, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpSize:
			return aero.MapSizeOp(in.BinName, toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpGetByKeyList:
			keys := toListOfValueIfcs(in.Args[0])
			if len(keys) == 1 {
				return aero.MapGetByKeyOp(in.BinName, keys[0], toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
			}
			return aero.MapGetByKeyListOp(in.BinName, keys, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpGetByKeyRange:
			begin := toValue(in.Args[0])
			end := toValue(in.Args[1])
			return aero.MapGetByKeyRangeOp(in.BinName, begin, end, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpGetByKeyRelativeIndexRange:
			key := toValue(in.Args[0])
			index := int(in.Args[1].GetI())
			return aero.MapGetByKeyRelativeIndexRangeOp(in.BinName, key, index, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpGetByKeyRelativeIndexRangeCount:
			key := toValue(in.Args[0])
			index := int(in.Args[1].GetI())
			count := int(in.Args[2].GetI())
			return aero.MapGetByKeyRelativeIndexRangeCountOp(in.BinName, key, index, count, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpGetByValueList:
			values := toListOfValueIfcs(in.Args[0])
			if len(values) == 1 {
				return aero.MapGetByValueOp(in.BinName, values[0], toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
			}
			return aero.MapGetByValueListOp(in.BinName, values, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpGetByValueRange:
			begin := toValue(in.Args[0])
			end := toValue(in.Args[1])
			return aero.MapGetByValueRangeOp(in.BinName, begin, end, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpGetByValueRelativeRankRange:
			value := toValue(in.Args[0])
			rank := int(in.Args[1].GetI())
			return aero.MapGetByValueRelativeRankRangeOp(in.BinName, value, rank, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpGetByValueRelativeRankRangeCount:
			value := toValue(in.Args[0])
			rank := int(in.Args[1].GetI())
			count := int(in.Args[2].GetI())
			return aero.MapGetByValueRelativeRankRangeCountOp(in.BinName, value, rank, count, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpGetByIndex:
			index := int(in.Args[0].GetI())
			return aero.MapGetByIndexOp(in.BinName, index, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpGetByIndexRange:
			index := int(in.Args[0].GetI())
			return aero.MapGetByIndexRangeOp(in.BinName, index, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpGetByIndexRangeCount:
			index := int(in.Args[0].GetI())
			count := int(in.Args[1].GetI())
			return aero.MapGetByIndexRangeCountOp(in.BinName, index, count, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpGetByRank:
			rank := int(in.Args[0].GetI())
			return aero.MapGetByRankOp(in.BinName, rank, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpGetByRankRange:
			rank := int(in.Args[0].GetI())
			return aero.MapGetByRankRangeOp(in.BinName, rank, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtMapCommandOp_CdtMapCommandOpGetByRankRangeCount:
			rank := int(in.Args[0].GetI())
			count := int(in.Args[1].GetI())
			return aero.MapGetByRankRangeCountOp(in.BinName, rank, count, toCdtMapReturnType(in.ReturnType), toCDTContexts(in.Ctx)...)
		}
	}

	return nil
}

func toCdtListPolicy(in *pb.CdtListPolicy) *aero.ListPolicy {
	if in != nil {
		return aero.NewListPolicy(aero.ListOrderType(in.GetOrder()), int(in.GetFlags()))
	}

	return aero.DefaultListPolicy()
}

func toCdtListOp(in *pb.CdtListOperation) *aero.Operation {
	if in != nil {
		switch in.Op {
		case pb.CdtListCommandOp_CdtListCommandOpCreate:
			listOrder := aero.ListOrderType(in.Args[0].GetI())
			pad := in.Args[1].GetB()
			persistedIndex := in.Args[2].GetB()
			if persistedIndex {
				return aero.ListCreateWithIndexOp(in.BinName, listOrder, pad, toCDTContexts(in.Ctx)...)
			}
			return aero.ListCreateOp(in.BinName, listOrder, pad, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpSetOrder:
			listOrder := aero.ListOrderType(in.Args[0].GetI())
			return aero.ListSetOrderOp(in.BinName, listOrder, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpAppend:
			values := toListOfValueIfcs(in.Args[0])
			return aero.ListAppendWithPolicyContextOp(toCdtListPolicy(in.Policy), in.BinName, toCDTContexts(in.Ctx), values...)
		case pb.CdtListCommandOp_CdtListCommandOpInsert:
			index := int(in.Args[0].GetI())
			values := toListOfValueIfcs(in.Args[1])
			return aero.ListInsertWithPolicyContextOp(toCdtListPolicy(in.Policy), in.BinName, index, toCDTContexts(in.Ctx), values...)
		case pb.CdtListCommandOp_CdtListCommandOpPop:
			index := int(in.Args[0].GetI())
			return aero.ListPopOp(in.BinName, index, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpPopRange:
			index := int(in.Args[0].GetI())
			count := int(in.Args[1].GetI())
			return aero.ListPopRangeOp(in.BinName, index, count, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpPopRangeFrom:
			index := int(in.Args[0].GetI())
			return aero.ListPopRangeFromOp(in.BinName, index, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpSet:
			index := int(in.Args[0].GetI())
			value := toValue(in.Args[1])
			return aero.ListSetWithPolicyOp(toCdtListPolicy(in.Policy), in.BinName, index, value, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpTrim:
			index := int(in.Args[0].GetI())
			count := int(in.Args[1].GetI())
			return aero.ListTrimOp(in.BinName, index, count, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpClear:
			return aero.ListClearOp(in.BinName, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpIncrement:
			index := int(in.Args[0].GetI())
			value := toValue(in.Args[1])
			return aero.ListIncrementWithPolicyOp(toCdtListPolicy(in.Policy), in.BinName, index, value, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpSize:
			return aero.ListSizeOp(in.BinName, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpSort:
			sortFlags := aero.ListSortFlags(in.Args[0].GetI())
			return aero.ListSortOp(in.BinName, sortFlags, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpGetByValueList:
			values := toListOfValueIfcs(in.Args[0])
			if len(values) == 1 {
				return aero.ListGetByValueOp(in.BinName, values[0], aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
			}
			return aero.ListGetByValueListOp(in.BinName, values, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpGetByValueRange:
			begin := toValue(in.Args[0])
			end := toValue(in.Args[1])
			return aero.ListGetByValueRangeOp(in.BinName, begin, end, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpGetByIndex:
			index := int(in.Args[0].GetI())
			return aero.ListGetByIndexOp(in.BinName, index, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpGetByIndexRange:
			index := int(in.Args[0].GetI())
			return aero.ListGetByIndexRangeOp(in.BinName, index, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpGetByIndexRangeCount:
			index := int(in.Args[0].GetI())
			count := int(in.Args[1].GetI())
			return aero.ListGetByIndexRangeCountOp(in.BinName, index, count, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpGetByRank:
			rank := int(in.Args[0].GetI())
			return aero.ListGetByRankOp(in.BinName, rank, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpGetByRankRange:
			rank := int(in.Args[0].GetI())
			return aero.ListGetByRankRangeOp(in.BinName, rank, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpGetByRankRangeCount:
			rank := int(in.Args[0].GetI())
			count := int(in.Args[1].GetI())
			return aero.ListGetByRankRangeCountOp(in.BinName, rank, count, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpGetByValueRelativeRankRange:
			value := toValue(in.Args[0])
			rank := int(in.Args[1].GetI())
			return aero.ListGetByValueRelativeRankRangeOp(in.BinName, value, rank, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpGetByValueRelativeRankRangeCount:
			value := toValue(in.Args[0])
			rank := int(in.Args[1].GetI())
			count := int(in.Args[2].GetI())
			return aero.ListGetByValueRelativeRankRangeCountOp(in.BinName, value, rank, count, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpRemoveByValueList:
			values := toListOfValueIfcs(in.Args[0])
			if len(values) == 1 {
				return aero.ListRemoveByValueOp(in.BinName, values[0], aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
			}
			return aero.ListRemoveByValueListOp(in.BinName, values, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpRemoveByValueRange:
			begin := toValue(in.Args[0])
			end := toValue(in.Args[1])
			return aero.ListRemoveByValueRangeOp(in.BinName, aero.ListReturnType(*in.ReturnType), begin, end, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpRemoveByValueRelativeRankRange:
			value := toValue(in.Args[0])
			rank := int(in.Args[1].GetI())
			return aero.ListRemoveByValueRelativeRankRangeOp(in.BinName, aero.ListReturnType(*in.ReturnType), value, rank, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpRemoveByValueRelativeRankRangeCount:
			value := toValue(in.Args[0])
			rank := int(in.Args[1].GetI())
			count := int(in.Args[2].GetI())
			return aero.ListRemoveByValueRelativeRankRangeCountOp(in.BinName, aero.ListReturnType(*in.ReturnType), value, rank, count, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpRemoveRange:
			index := int(in.Args[0].GetI())
			count := int(in.Args[1].GetI())
			return aero.ListRemoveRangeOp(in.BinName, index, count, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpRemoveRangeFrom:
			index := int(in.Args[0].GetI())
			return aero.ListRemoveRangeFromOp(in.BinName, index, toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpRemoveByIndex:
			index := int(in.Args[0].GetI())
			return aero.ListRemoveByIndexOp(in.BinName, index, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpRemoveByIndexRange:
			index := int(in.Args[0].GetI())
			return aero.ListRemoveByIndexRangeOp(in.BinName, index, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpRemoveByIndexRangeCount:
			index := int(in.Args[0].GetI())
			count := int(in.Args[1].GetI())
			return aero.ListRemoveByIndexRangeCountOp(in.BinName, index, count, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpRemoveByRank:
			rank := int(in.Args[0].GetI())
			return aero.ListRemoveByRankOp(in.BinName, rank, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpRemoveByRankRange:
			rank := int(in.Args[0].GetI())
			return aero.ListRemoveByRankRangeOp(in.BinName, rank, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		case pb.CdtListCommandOp_CdtListCommandOpRemoveByRankRangeCount:
			rank := int(in.Args[0].GetI())
			count := int(in.Args[1].GetI())
			return aero.ListRemoveByRankRangeCountOp(in.BinName, rank, count, aero.ListReturnType(*in.ReturnType), toCDTContexts(in.Ctx)...)
		}
	}

	return nil
}

func toHLLPolicy(in *pb.CdtHLLPolicy) *aero.HLLPolicy {
	if in != nil {
		return aero.NewHLLPolicy(int(in.Flags))
	}

	return aero.DefaultHLLPolicy()
}

func toListOfValueIfcs(in *pb.Value) []interface{} {
	l := in.GetL().L
	res := make([]interface{}, len(l))
	for i := range l {
		res[i] = toValue(l[i])
	}
	return res
}

func toListOfValues(in *pb.Value) []aero.Value {
	l := in.GetL().L
	res := make([]aero.Value, len(l))
	for i := range l {
		res[i] = toValue(l[i])
	}
	return res
}

func toListOfHLLValues(in *pb.Value) []aero.HLLValue {
	l := in.GetL().L
	res := make([]aero.HLLValue, len(l))
	for i := range l {
		res[i] = toValue(l[i]).(aero.HLLValue)
	}
	return res
}

func toCdtHLLOp(in *pb.CdtHLLOperation) *aero.Operation {
	if in != nil {
		switch in.Op {
		case pb.CdtHLLCommandOp_CdtHLLCommandOpInit:
			indexBitCount := int(in.Args[0].GetI())
			minHashBitCount := int(in.Args[1].GetI())
			return aero.HLLInitOp(toHLLPolicy(in.Policy), in.BinName, indexBitCount, minHashBitCount)
		case pb.CdtHLLCommandOp_CdtHLLCommandOpAdd:
			list := toListOfValues(in.Args[0])
			indexBitCount := int(in.Args[1].GetI())
			minHashBitCount := int(in.Args[2].GetI())
			return aero.HLLAddOp(toHLLPolicy(in.Policy), in.BinName, list, indexBitCount, minHashBitCount)
		case pb.CdtHLLCommandOp_CdtHLLCommandOpSetUnion:
			list := toListOfHLLValues(in.Args[0])
			return aero.HLLSetUnionOp(toHLLPolicy(in.Policy), in.BinName, list)
		case pb.CdtHLLCommandOp_CdtHLLCommandOpRefreshCount:
			return aero.HLLRefreshCountOp(in.BinName)
		case pb.CdtHLLCommandOp_CdtHLLCommandOpFold:
			indexBitCount := int(in.Args[0].GetI())
			return aero.HLLFoldOp(in.BinName, indexBitCount)
		case pb.CdtHLLCommandOp_CdtHLLCommandOpGetCount:
			return aero.HLLGetCountOp(in.BinName)
		case pb.CdtHLLCommandOp_CdtHLLCommandOpGetUnion:
			list := toListOfHLLValues(in.Args[0])
			return aero.HLLGetUnionOp(in.BinName, list)
		case pb.CdtHLLCommandOp_CdtHLLCommandOpGetUnionCount:
			list := toListOfHLLValues(in.Args[0])
			return aero.HLLGetUnionCountOp(in.BinName, list)
		case pb.CdtHLLCommandOp_CdtHLLCommandOpGetIntersectCount:
			list := toListOfHLLValues(in.Args[0])
			return aero.HLLGetIntersectCountOp(in.BinName, list)
		case pb.CdtHLLCommandOp_CdtHLLCommandOpGetSimilarity:
			list := toListOfHLLValues(in.Args[0])
			return aero.HLLGetSimilarityOp(in.BinName, list)
		case pb.CdtHLLCommandOp_CdtHLLCommandOpDescribe:
			return aero.HLLDescribeOp(in.BinName)
		}
	}

	return nil
}

func toBitwisePolicy(in *pb.CdtBitwisePolicy) *aero.BitPolicy {
	if in != nil {
		return aero.NewBitPolicy(int(in.Flags))
	}

	return aero.DefaultBitPolicy()
}

func toCdtBitwiseOp(in *pb.CdtBitwiseOperation) *aero.Operation {
	if in != nil {
		switch in.Op {
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpResize:
			byteSize := int(in.Args[0].GetI())
			resizeFlags := aero.BitResizeFlags(in.Args[1].GetI())
			return aero.BitResizeOp(toBitwisePolicy(in.Policy), in.BinName, byteSize, resizeFlags, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpInsert:
			byteOffset := int(in.Args[0].GetI())
			value := in.Args[1].GetBlob()
			return aero.BitInsertOp(toBitwisePolicy(in.Policy), in.BinName, byteOffset, value, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpRemove:
			byteOffset := int(in.Args[0].GetI())
			byteSize := int(in.Args[1].GetI())
			return aero.BitRemoveOp(toBitwisePolicy(in.Policy), in.BinName, byteOffset, byteSize, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpSet:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			value := in.Args[2].GetBlob()
			return aero.BitSetOp(toBitwisePolicy(in.Policy), in.BinName, bitOffset, bitSize, value, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpOr:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			value := in.Args[2].GetBlob()
			return aero.BitOrOp(toBitwisePolicy(in.Policy), in.BinName, bitOffset, bitSize, value, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpXor:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			value := in.Args[2].GetBlob()
			return aero.BitXorOp(toBitwisePolicy(in.Policy), in.BinName, bitOffset, bitSize, value, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpAnd:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			value := in.Args[2].GetBlob()
			return aero.BitAndOp(toBitwisePolicy(in.Policy), in.BinName, bitOffset, bitSize, value, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpNot:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			return aero.BitNotOp(toBitwisePolicy(in.Policy), in.BinName, bitOffset, bitSize, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpLShift:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			shift := int(in.Args[2].GetI())
			return aero.BitLShiftOp(toBitwisePolicy(in.Policy), in.BinName, bitOffset, bitSize, shift, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpRShift:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			shift := int(in.Args[2].GetI())
			return aero.BitRShiftOp(toBitwisePolicy(in.Policy), in.BinName, bitOffset, bitSize, shift, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpAdd:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			value := in.Args[2].GetI()
			signed := in.Args[3].GetB()
			action := aero.BitOverflowAction(in.Args[3].GetI())
			return aero.BitAddOp(toBitwisePolicy(in.Policy), in.BinName, bitOffset, bitSize, value, signed, action, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpSubtract:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			value := in.Args[2].GetI()
			signed := in.Args[3].GetB()
			action := aero.BitOverflowAction(in.Args[3].GetI())
			return aero.BitSubtractOp(toBitwisePolicy(in.Policy), in.BinName, bitOffset, bitSize, value, signed, action, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpSetInt:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			value := in.Args[2].GetI()
			return aero.BitSetIntOp(toBitwisePolicy(in.Policy), in.BinName, bitOffset, bitSize, value, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpGet:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			return aero.BitGetOp(in.BinName, bitOffset, bitSize, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpCount:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			return aero.BitCountOp(in.BinName, bitOffset, bitSize, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpLScan:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			value := in.Args[2].GetB()
			return aero.BitLScanOp(in.BinName, bitOffset, bitSize, value, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpRScan:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			value := in.Args[2].GetB()
			return aero.BitRScanOp(in.BinName, bitOffset, bitSize, value, toCDTContexts(in.Ctx)...)
		case pb.CdtBitwiseCommandOp_CdtBitwiseCommandOpGetInt:
			bitOffset := int(in.Args[0].GetI())
			bitSize := int(in.Args[1].GetI())
			signed := in.Args[2].GetB()
			return aero.BitGetIntOp(in.BinName, bitOffset, bitSize, signed, toCDTContexts(in.Ctx)...)
		}
	}

	return nil
}

func toOp(in *pb.Operation) *aero.Operation {
	switch in := in.Op.(type) {
	case *pb.Operation_Std:
		return toStdOp(in.Std)
	case *pb.Operation_Map:
		return toCdtMapOp(in.Map)
	case *pb.Operation_List:
		return toCdtListOp(in.List)
	case *pb.Operation_Hll:
		return toCdtHLLOp(in.Hll)
	case *pb.Operation_Bitwise:
		return toCdtBitwiseOp(in.Bitwise)
	}
	panic(UNREACHABLE)
}

func toBins(in []*pb.Bin) (res []*aero.Bin) {
	if len(in) > 0 {
		res = make([]*aero.Bin, len(in))
	}

	for i := range in {
		res[i] = aero.NewBin(in[i].Name, toValue(in[i].Value))
	}

	return res
}

func toBinMap(in []*pb.Bin) (res aero.BinMap) {
	if len(in) > 0 {
		res = make(aero.BinMap, len(in))
	}

	for i := range in {
		res[in[i].Name] = toValue(in[i].Value)
	}

	return res
}

func toKey(in *pb.Key) *aero.Key {
	if in != nil {
		k, err := aero.NewKey(in.GetNamespace(), in.GetSet(), toValue(in.Value))
		if err != nil {
			panic(err)
		}
		return k
	}
	return nil
}

func toValues(in []*pb.Value) (res []aero.Value) {
	for i := range in {
		res = append(res, toValue(in[i]))
	}

	return res
}

func toValue(in *pb.Value) aero.Value {
	if in == nil {
		return aero.NewNullValue()
	}

	if _, ok := in.V.(*pb.Value_Nil); ok {
		return aero.NewNullValue()
	}

	if v, ok := in.V.(*pb.Value_I); ok {
		return aero.IntegerValue(v.I)
	}

	if v, ok := in.V.(*pb.Value_S); ok {
		return aero.StringValue(v.S)
	}

	if v, ok := in.V.(*pb.Value_F); ok {
		return aero.FloatValue(v.F)
	}

	if v, ok := in.V.(*pb.Value_B); ok {
		return aero.BoolValue(v.B)
	}

	if v, ok := in.V.(*pb.Value_Blob); ok {
		return aero.BytesValue(v.Blob)
	}

	if v, ok := in.V.(*pb.Value_Geo); ok {
		return aero.GeoJSONValue(v.Geo)
	}

	if v, ok := in.V.(*pb.Value_Hll); ok {
		return aero.HLLValue(v.Hll)
	}

	if _, ok := in.V.(*pb.Value_Wildcard); ok {
		return aero.NewWildCardValue()
	}

	if _, ok := in.V.(*pb.Value_Infinity); ok {
		return aero.NewInfinityValue()
	}

	if j, ok := in.V.(*pb.Value_Json); ok {
		jsn := j.Json.GetJ()
		j := make(map[string]interface{}, len(jsn))
		for i := range jsn {
			j[jsn[i].K] = toValue(jsn[i].V)
		}
		return aero.JsonValue(j)
	}

	if m, ok := in.V.(*pb.Value_M); ok {
		mp := m.M.GetM()
		m := make(map[interface{}]interface{}, len(mp))
		for i := range mp {
			m[toValue(mp[i].K)] = toValue(mp[i].V)
		}
		return aero.MapValue(m)
	}

	if l, ok := in.V.(*pb.Value_L); ok {
		lst := l.L.L
		l := make([]interface{}, len(lst))
		for i := range lst {
			l[i] = toValue(lst[i])
		}
		return aero.ListValue(l)
	}

	panic(UNREACHABLE)
}

func toListValue(in *pb.Value) []aero.Value {
	lst := in.GetL().L
	if len(lst) > 0 {
		l := make([]aero.Value, len(lst))
		for i := range lst {
			l[i] = toValue(lst[i])
		}
		return l
	}

	panic(UNREACHABLE)
}

func fromBatchRecords(in []aero.BatchRecordIfc) (res []*pb.BatchRecord) {
	for i := range in {
		res = append(res, fromBatchRecord(in[i]))
	}
	return res
}

func fromBatchRecord(in aero.BatchRecordIfc) *pb.BatchRecord {
	if in != nil {
		br := in.BatchRec()
		return &pb.BatchRecord{
			Key:    fromKey(br.Key),
			Record: fromRecord(br.Record),
			Error:  fromError2(br.Err, br.InDoubt),
		}
	}
	return nil
}

func fromError(in aero.Error) *pb.Error {
	if in != nil {
		ae := in.(*aero.AerospikeError)
		return &pb.Error{
			ResultCode: int32(ae.ResultCode),
			InDoubt:    ae.IsInDoubt(),
		}
	}
	return nil
}

func fromError2(in aero.Error, inDoubt bool) *pb.Error {
	if in != nil {
		ae := in.(*aero.AerospikeError)
		return &pb.Error{
			ResultCode: int32(ae.ResultCode),
			InDoubt:    inDoubt,
		}
	}

	if inDoubt {
		return &pb.Error{
			ResultCode: 0,
			InDoubt:    inDoubt,
		}
	}

	return nil
}

func fromKey(in *aero.Key) *pb.Key {
	if in != nil {
		ns := in.Namespace()
		set := in.SetName()
		return &pb.Key{
			Digest:    in.Digest(),
			Namespace: &ns,
			Set:       &set,
			Value:     fromValue(in.Value()),
		}
	}
	return nil
}

func fromRecord(in *aero.Record) *pb.Record {
	if in != nil {
		return &pb.Record{
			Key:        fromKey(in.Key),
			Generation: in.Generation,
			Expiration: in.Expiration,
			Bins:       fromBins(in.Bins),
		}
	}
	return nil
}

func fromBins(in aero.BinMap) map[string]*pb.Value {
	if len(in) > 0 {
		res := make(map[string]*pb.Value, len(in))
		for k, v := range in {
			res[k] = fromValue(v)
		}
		return res
	}
	return nil
}

func fromValue(in any) *pb.Value {
	if in == nil {
		return &pb.Value{V: &pb.Value_Nil{Nil: true}}
	}

	switch v := in.(type) {
	case int:
		i64 := int64(v)
		return &pb.Value{V: &pb.Value_I{I: i64}}
	case float64:
		return &pb.Value{V: &pb.Value_F{F: v}}
	case string:
		return &pb.Value{V: &pb.Value_S{S: v}}
	case bool:
		return &pb.Value{V: &pb.Value_B{B: v}}
	case []byte:
		return &pb.Value{V: &pb.Value_Blob{Blob: v}}
	case []any:
		l := make([]*pb.Value, len(v))
		for i := range v {
			l[i] = fromValue(v[i])
		}
		return &pb.Value{V: &pb.Value_L{L: &pb.List{L: l}}}
	case map[any]any:
		m := make([]*pb.MapEntry, len(v))
		for k, v := range v {
			m = append(m, &proto.MapEntry{K: fromValue(k), V: fromValue(v)})
		}
		return &pb.Value{V: &pb.Value_M{M: &pb.Map{M: m}}}
	case []aero.MapPair:
		m := make([]*pb.MapEntry, len(v))
		for _, mp := range v {
			m = append(m, &proto.MapEntry{K: fromValue(mp.Key), V: fromValue(mp.Value)})
		}
		return &pb.Value{V: &pb.Value_M{M: &pb.Map{M: m}}}
	case aero.NullValue:
		return &pb.Value{V: &pb.Value_Nil{Nil: true}}
	case aero.IntegerValue:
		return &pb.Value{V: &pb.Value_I{I: int64(v)}}
	case aero.FloatValue:
		return &pb.Value{V: &pb.Value_F{F: float64(v)}}
	case aero.StringValue:
		return &pb.Value{V: &pb.Value_S{S: string(v)}}
	case aero.BoolValue:
		return &pb.Value{V: &pb.Value_B{B: bool(v)}}
	case aero.BytesValue:
		return &pb.Value{V: &pb.Value_Blob{Blob: []byte(v)}}
	case aero.JsonValue:
		m := make([]*pb.JsonEntry, len(v))
		for k, v := range v {
			m = append(m, &proto.JsonEntry{K: k, V: fromValue(v)})
		}
		return &pb.Value{V: &pb.Value_Json{Json: &pb.Json{J: m}}}
	case aero.HLLValue:
		return &pb.Value{V: &pb.Value_Hll{Hll: v.GetObject().([]byte)}}
	case aero.GeoJSONValue:
		return &pb.Value{V: &pb.Value_Geo{Geo: v.GetObject().(string)}}
	case aero.WildCardValue:
		return &pb.Value{V: &pb.Value_Wildcard{Wildcard: true}}
	case aero.InfinityValue:
		return &pb.Value{V: &pb.Value_Infinity{Infinity: true}}
	}

	panic(UNREACHABLE)
}

func toExpressions(in []*pb.Expression) (res []*aero.Expression) {
	for i := range in {
		res = append(res, toExpression(in[i]))
	}
	return res
}

func toExpression(in *pb.Expression) *aero.Expression {
	if in == nil {
		return nil
	}

	if in.Cmd == nil {
		val := toValue(in.Val)
		switch v := val.(type) {
		case aero.NullValue:
			return aero.ExpNilValue()
		case aero.IntegerValue:
			return aero.ExpIntVal(int64(v))
		case aero.FloatValue:
			return aero.ExpFloatVal(float64(v))
		case aero.StringValue:
			return aero.ExpStringVal(string(v))
		case aero.BoolValue:
			return aero.ExpBoolVal(bool(v))
		case aero.BytesValue:
			return aero.ExpBlobVal([]byte(v))
		case aero.ListValue:
			return aero.ExpListVal(toListValue(in.Val)...)
		case aero.MapValue:
			return aero.ExpMapVal(toValue(in.Val).(aero.MapValue))
			// case .aero.ExpJsonValue:
			// 	return aero.ExpJsonVal(toValue(in.Val.Json))
		case aero.GeoJSONValue:
			return aero.ExpGeoVal(string(v))
			// case .aero.ExpHllValue:
			// 	return aero.ExpHllVal(toValue(in.Val))
		case aero.WildCardValue:
			return aero.ExpWildCardValue()
		case aero.InfinityValue:
			return aero.ExpInfinityValue()
		}
	}

	switch *in.Cmd {
	case pb.ExpOp_ExpOpUnknown:
		return aero.ExpUnknown()
	case pb.ExpOp_ExpOpEq:
		return aero.ExpEq(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpNe:
		return aero.ExpNotEq(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpGt:
		return aero.ExpGreater(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpGe:
		return aero.ExpGreaterEq(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpLt:
		return aero.ExpLess(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpLe:
		return aero.ExpLessEq(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpRegex:
		return aero.ExpRegexCompare(toValue(in.Val).String(), aero.ExpRegexFlags(*in.Flags), toExpression(in.Bin))
	case pb.ExpOp_ExpOpGeo:
		return aero.ExpGeoCompare(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpAnd:
		return aero.ExpAnd(toExpressions(in.Exps)...)
	case pb.ExpOp_ExpOpOr:
		return aero.ExpOr(toExpressions(in.Exps)...)
	case pb.ExpOp_ExpOpNot:
		return aero.ExpNot(toExpression(in.Exps[0]))
	case pb.ExpOp_ExpOpExclusive:
		return aero.ExpExclusive(toExpressions(in.Exps)...)
	case pb.ExpOp_ExpOpAdd:
		return aero.ExpNumAdd(toExpressions(in.Exps)...)
	case pb.ExpOp_ExpOpSub:
		return aero.ExpNumSub(toExpressions(in.Exps)...)
	case pb.ExpOp_ExpOpMul:
		return aero.ExpNumMul(toExpressions(in.Exps)...)
	case pb.ExpOp_ExpOpDiv:
		return aero.ExpNumDiv(toExpressions(in.Exps)...)
	case pb.ExpOp_ExpOpPow:
		return aero.ExpNumPow(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpLog:
		return aero.ExpNumLog(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpMod:
		return aero.ExpNumMod(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpAbs:
		return aero.ExpNumAbs(toExpression(in.Exps[0]))
	case pb.ExpOp_ExpOpFloor:
	case pb.ExpOp_ExpOpCeil:
		return aero.ExpNumCeil(toExpression(in.Exps[0]))
	case pb.ExpOp_ExpOpToInt:
		return aero.ExpToInt(toExpression(in.Exps[0]))
	case pb.ExpOp_ExpOpToFloat:
		return aero.ExpToFloat(toExpression(in.Exps[0]))
	case pb.ExpOp_ExpOpIntAnd:
		return aero.ExpIntAnd(toExpressions(in.Exps)...)
	case pb.ExpOp_ExpOpIntOr:
		return aero.ExpIntOr(toExpressions(in.Exps)...)
	case pb.ExpOp_ExpOpIntXor:
		return aero.ExpIntXor(toExpressions(in.Exps)...)
	case pb.ExpOp_ExpOpIntNot:
		return aero.ExpIntNot(toExpression(in.Exps[0]))
	case pb.ExpOp_ExpOpIntLShift:
		return aero.ExpIntLShift(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpIntRShift:
		return aero.ExpIntRShift(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpIntARShift:
		return aero.ExpIntARShift(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpIntCount:
		return aero.ExpIntCount(toExpression(in.Exps[0]))
	case pb.ExpOp_ExpOpIntLScan:
		return aero.ExpIntLScan(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpIntRScan:
		return aero.ExpIntRScan(toExpression(in.Exps[0]), toExpression(in.Exps[1]))
	case pb.ExpOp_ExpOpMin:
		return aero.ExpMin(toExpressions(in.Exps)...)
	case pb.ExpOp_ExpOpMax:
		return aero.ExpMax(toExpressions(in.Exps)...)
	case pb.ExpOp_ExpOpDigestModulo:
		return aero.ExpDigestModulo(in.Val.V.(*pb.Value_I).I)
	case pb.ExpOp_ExpOpDeviceSize:
		return aero.ExpDeviceSize()
	case pb.ExpOp_ExpOpLastUpdate:
		return aero.ExpLastUpdate()
	case pb.ExpOp_ExpOpSinceUpdate:
		return aero.ExpSinceUpdate()
	case pb.ExpOp_ExpOpVoidTime:
		return aero.ExpVoidTime()
	case pb.ExpOp_ExpOpTtl:
		return aero.ExpTTL()
	case pb.ExpOp_ExpOpSetName:
		return aero.ExpSetName()
	case pb.ExpOp_ExpOpKeyExists:
		return aero.ExpKeyExists()
	case pb.ExpOp_ExpOpIsTombstone:
		return aero.ExpIsTombstone()
	case pb.ExpOp_ExpOpMemorySize:
		return aero.ExpMemorySize()
	case pb.ExpOp_ExpOpRecordSize:
		return aero.ExpRecordSize()
	case pb.ExpOp_ExpOpKey:
		return aero.ExpKey(aero.ExpType(in.Val.V.(*pb.Value_I).I))
	case pb.ExpOp_ExpOpBin:
		switch *in.Module {
		case pb.ExpType_ExpTypeBool:
			return aero.ExpBoolBin(in.Val.V.(*pb.Value_S).S)
		case pb.ExpType_ExpTypeInt:
			return aero.ExpIntBin(in.Val.V.(*pb.Value_S).S)
		case pb.ExpType_ExpTypeString:
			return aero.ExpStringBin(in.Val.V.(*pb.Value_S).S)
		case pb.ExpType_ExpTypeList:
			return aero.ExpListBin(in.Val.V.(*pb.Value_S).S)
		case pb.ExpType_ExpTypeMap:
			return aero.ExpMapBin(in.Val.V.(*pb.Value_S).S)
		case pb.ExpType_ExpTypeBlob:
			return aero.ExpBlobBin(in.Val.V.(*pb.Value_S).S)
		case pb.ExpType_ExpTypeFloat:
			return aero.ExpFloatBin(in.Val.V.(*pb.Value_S).S)
		case pb.ExpType_ExpTypeGeo:
			return aero.ExpGeoBin(in.Val.V.(*pb.Value_S).S)
			// case pb.ExpType_ExpTypeHll:
			// 	return aero.ExpHllBin(toValue(in.Val).String())
		}
		panic(UNREACHABLE)
	case pb.ExpOp_ExpOpBinType:
		return aero.ExpBinType(toValue(in.Val).String())
	case pb.ExpOp_ExpOpCond:
		return aero.ExpCond(toExpressions(in.Exps)...)
	case pb.ExpOp_ExpOpVar:
		return aero.ExpVar(toValue(in.Val).String())
	case pb.ExpOp_ExpOpLet:
		return aero.ExpLet(toExpressions(in.Exps)...)
	case pb.ExpOp_ExpOpQuoted:
		return aero.ExpListVal(toValue(in.Val))
	}
	panic(UNREACHABLE)
}
