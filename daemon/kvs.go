package main

import (
	"context"
	"time"

	aero "github.com/aerospike/aerospike-client-go/v7"

	"github.com/aerospike/php-client/asld/proto"
	pb "github.com/aerospike/php-client/asld/proto"
)

const UNREACHABLE = "UNREACHABLE"

type server struct {
	pb.UnimplementedKVSServer

	client *aero.Client
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

func toOp(in *pb.Operation) *aero.Operation {
	if in != nil {
		switch in.OpType {
		case pb.OperationType_OperationTypeRead:
			if in.BinName != nil {
				return aero.GetBinOp(*in.BinName)
			}
			return aero.GetOp()
		case pb.OperationType_OperationTypeReadHeader:
			return aero.GetHeaderOp()
		case pb.OperationType_OperationTypeWrite:
			return aero.PutOp(aero.NewBin(*in.BinName, toValue(in.BinValue)))
		// case pb.OperationType_OperationTypeCdtRead:
		// case pb.OperationType_OperationTypeCdtModify:
		// case pb.OperationType_OperationTypeMapRead:
		// case pb.OperationType_OperationTypeMapModify:
		case pb.OperationType_OperationTypeAdd:
			return aero.AddOp(aero.NewBin(*in.BinName, toValue(in.BinValue)))
		// case pb.OperationType_OperationTypeExpRead:
		// case pb.OperationType_OperationTypeExpModify:
		case pb.OperationType_OperationTypeAppend:
			return aero.AppendOp(aero.NewBin(*in.BinName, toValue(in.BinValue)))
		case pb.OperationType_OperationTypePrepend:
			return aero.PrependOp(aero.NewBin(*in.BinName, toValue(in.BinValue)))
		case pb.OperationType_OperationTypeTouch:
			return aero.TouchOp()
		// case pb.OperationType_OperationTypeBitRead:
		// case pb.OperationType_OperationTypeBitModify:
		case pb.OperationType_OperationTypeDelete:
			return aero.DeleteOp()
			// case pb.OperationType_OperationTypeHllRead:
			// case pb.OperationType_OperationTypeHllModify:
		}
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
