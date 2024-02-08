package main

import (
	"context"
	"log"
	"reflect"

	aero "github.com/aerospike/aerospike-client-go/v7"

	"github.com/aerospike/php-client/asld/proto"
	pb "github.com/aerospike/php-client/asld/proto"
)

type server struct {
	pb.UnimplementedKVSServer
}

func (s *server) Get(ctx context.Context, in *pb.AerospikeGetRequest) (*pb.AerospikeSingleResponse, error) {
	policy := toReadPolicy(in.Policy)
	key := toKey(in.Key)
	rec, err := client.Get(policy, key, in.BinNames...)
	if err != nil {
		inDoubt := err.IsInDoubt()
		return &pb.AerospikeSingleResponse{
			Error: &pb.Error{
				ResultCode: 0, // TODO: return result code
				InDoubt:    inDoubt,
			},
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
	rec, err := client.GetHeader(policy, key)
	if err != nil {
		inDoubt := err.IsInDoubt()
		return &pb.AerospikeSingleResponse{
			Error: &pb.Error{
				ResultCode: 0, // TODO: return result code
				InDoubt:    inDoubt,
			},
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
	exists, err := client.Exists(policy, key)
	if err != nil {
		inDoubt := err.IsInDoubt()
		return &pb.AerospikeExistsResponse{
			Error: &pb.Error{
				ResultCode: 0, // TODO: return result code
				InDoubt:    inDoubt,
			},
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
	err := client.PutBins(policy, key, toBins(in.Bins)...)
	if err != nil {
		inDoubt := err.IsInDoubt()
		return &pb.Error{
			ResultCode: 0, // TODO: return result code
			InDoubt:    inDoubt,
		}, nil
	}

	return &pb.Error{
		ResultCode: 0,
	}, nil
}

func (s *server) Add(ctx context.Context, in *pb.AerospikePutRequest) (*pb.Error, error) {
	policy := toWritePolicy(in.Policy)
	key := toKey(in.Key)
	err := client.AddBins(policy, key, toBins(in.Bins)...)
	if err != nil {
		inDoubt := err.IsInDoubt()
		return &pb.Error{
			ResultCode: 0, // TODO: return result code
			InDoubt:    inDoubt,
		}, nil
	}

	return &pb.Error{
		ResultCode: 0,
	}, nil
}

func (s *server) Append(ctx context.Context, in *pb.AerospikePutRequest) (*pb.Error, error) {
	policy := toWritePolicy(in.Policy)
	key := toKey(in.Key)
	err := client.AppendBins(policy, key, toBins(in.Bins)...)
	if err != nil {
		inDoubt := err.IsInDoubt()
		return &pb.Error{
			ResultCode: 0, // TODO: return result code
			InDoubt:    inDoubt,
		}, nil
	}

	return &pb.Error{
		ResultCode: 0,
	}, nil
}

func (s *server) Prepend(ctx context.Context, in *pb.AerospikePutRequest) (*pb.Error, error) {
	policy := toWritePolicy(in.Policy)
	key := toKey(in.Key)
	err := client.PrependBins(policy, key, toBins(in.Bins)...)
	if err != nil {
		inDoubt := err.IsInDoubt()
		return &pb.Error{
			ResultCode: 0, // TODO: return result code
			InDoubt:    inDoubt,
		}, nil
	}

	return &pb.Error{
		ResultCode: 0,
	}, nil
}

func (s *server) Delete(ctx context.Context, in *pb.AerospikeDeleteRequest) (*pb.AerospikeDeleteResponse, error) {
	policy := toWritePolicy(in.Policy)
	key := toKey(in.Key)
	existed, err := client.Delete(policy, key)
	if err != nil {
		inDoubt := err.IsInDoubt()
		return &pb.AerospikeDeleteResponse{
			Error: &pb.Error{
				ResultCode: 0, // TODO: return result code
				InDoubt:    inDoubt,
			},
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
	err := client.Touch(policy, key)
	if err != nil {
		inDoubt := err.IsInDoubt()
		return &pb.Error{
			ResultCode: 0, // TODO: return result code
			InDoubt:    inDoubt,
		}, nil
	}

	return &pb.Error{
		ResultCode: 0,
	}, nil
}

func (s *server) BatchOperate(ctx context.Context, in *pb.AerospikeBatchOperateRequest) (*pb.AerospikeBatchOperateResponse, error) {
	brecs := toBatchRecords(in.Records)
	err := client.BatchOperate(toBatchPolicy(in.Policy), brecs)
	if err != nil {
		inDoubt := err.IsInDoubt()
		return &pb.AerospikeBatchOperateResponse{
			Error: &pb.Error{
				ResultCode: 0, // TODO: return result code
				InDoubt:    inDoubt,
			},
			Records: fromBatchRecords(brecs),
		}, nil
	}

	return &pb.AerospikeBatchOperateResponse{
		Records: fromBatchRecords(brecs),
	}, nil
}

func (s *server) CreateIndex(ctx context.Context, in *pb.AerospikeCreateIndexRequest) (*pb.AerospikeCreateIndexResponse, error) {
	// TODO(Khosrow): return the task
	_, err := client.CreateComplexIndex(toWritePolicy(in.Policy), in.Namespace, in.SetName, in.IndexName, in.BinName, toIndexType(in.IndexType), toIndexCollectionType(in.IndexCollectionType))
	if err != nil {
		inDoubt := err.IsInDoubt()
		return &pb.AerospikeCreateIndexResponse{
			Error: &pb.Error{
				ResultCode: 0, // TODO: return result code
				InDoubt:    inDoubt,
			},
		}, nil
	}

	return &pb.AerospikeCreateIndexResponse{Error: nil}, nil
}

func (s *server) DropIndex(ctx context.Context, in *pb.AerospikeDropIndexRequest) (*pb.AerospikeDropIndexResponse, error) {
	err := client.DropIndex(toWritePolicy(in.Policy), in.Namespace, in.SetName, in.IndexName)
	if err != nil {
		inDoubt := err.IsInDoubt()
		return &pb.AerospikeDropIndexResponse{
			Error: &pb.Error{
				ResultCode: 0, // TODO: return result code
				InDoubt:    inDoubt,
			},
		}, nil
	}

	return &pb.AerospikeDropIndexResponse{Error: nil}, nil
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

	panic("UNREACHABLE")
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

	panic("UNREACHABLE")
}

func toReadPolicy(in *pb.ReadPolicy) *aero.BasePolicy {
	if in != nil {
		return &aero.BasePolicy{}
	}
	return nil
}

func toWritePolicy(in *pb.WritePolicy) *aero.WritePolicy {
	if in != nil {
		return &aero.WritePolicy{}
	}
	return nil
}

func toBatchPolicy(in *pb.BatchPolicy) *aero.BatchPolicy {
	if in != nil {
		return &aero.BatchPolicy{}
	}
	return nil
}

func toBatchReadPolicy(in *pb.BatchReadPolicy) *aero.BatchReadPolicy {
	if in != nil {
		return &aero.BatchReadPolicy{}
	}
	return nil
}

func toBatchWritePolicy(in *pb.BatchWritePolicy) *aero.BatchWritePolicy {
	if in != nil {
		return &aero.BatchWritePolicy{}
	}
	return nil
}

func toBatchDeletePolicy(in *pb.BatchDeletePolicy) *aero.BatchDeletePolicy {
	if in != nil {
		return &aero.BatchDeletePolicy{}
	}
	return nil
}

func toBatchUDFPolicy(in *pb.BatchUDFPolicy) *aero.BatchUDFPolicy {
	if in != nil {
		return &aero.BatchUDFPolicy{}
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

	panic("UNREACHABLE")
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

	panic("UNREACHABLE")
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

	if in.Nil != nil {
		return aero.NewNullValue()
	}

	if in.I != nil {
		return aero.IntegerValue(*in.I)
	}

	if in.S != nil {
		return aero.StringValue(*in.S)
	}

	if in.F != nil {
		return aero.FloatValue(*in.F)
	}

	if in.B != nil {
		return aero.BoolValue(*in.B)
	}

	if in.Blob != nil {
		return aero.BytesValue(in.Blob)
	}

	if in.Geo != nil {
		return aero.GeoJSONValue(*in.Geo)
	}

	if in.Hll != nil {
		return aero.HLLValue(in.Hll)
	}

	if len(in.M) > 0 {
		m := make(map[interface{}]interface{}, len(in.M))
		for i := range in.M {
			m[toValue(in.M[i].K)] = toValue(in.M[i].V)
		}
		return aero.MapValue(m)
	}

	if len(in.L) > 0 {
		l := make([]interface{}, len(in.L))
		for i := range in.L {
			l[i] = toValue(in.L[i])
		}
		return aero.ListValue(l)
	}

	panic("UNREACHABLE")
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
			Error:  fromError(br.Err, br.InDoubt),
		}
	}
	return nil
}

func fromError(in aero.Error, inDoubt bool) *pb.Error {
	if in != nil {
		return &pb.Error{
			ResultCode: 0, // TODO: Return ResultCode
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
	switch v := in.(type) {
	case int:
		i64 := int64(v)
		return &pb.Value{I: &i64}
	case float64:
		return &pb.Value{F: &v}
	case string:
		return &pb.Value{S: &v}
	case bool:
		return &pb.Value{B: &v}
	case []byte:
		return &pb.Value{Blob: v}
	case []any:
		l := make([]*pb.Value, len(v))
		for i := range v {
			l[i] = fromValue(v[i])
		}
		return &pb.Value{L: l}
	case map[any]any:
		m := make([]*pb.MapEntry, len(v))
		for k, v := range v {
			m = append(m, &proto.MapEntry{K: fromValue(k), V: fromValue(v)})
		}
		return &pb.Value{M: m}
	case aero.IntegerValue:
		i64 := int64(v)
		return &pb.Value{I: &i64}
	case aero.FloatValue:
		f64 := float64(v)
		return &pb.Value{F: &f64}
	case aero.StringValue:
		s := string(v)
		return &pb.Value{S: &s}
	case aero.BoolValue:
		b := bool(v)
		return &pb.Value{B: &b}
	case aero.BytesValue:
		b := []byte(v)
		return &pb.Value{Blob: b}
	case aero.JsonValue:
		m := make([]*pb.JsonEntry, len(v))
		for k, v := range v {
			m = append(m, &proto.JsonEntry{K: k, V: fromValue(v)})
		}
		return &pb.Value{Json: m}
	case aero.NullValue:
		b := true
		return &pb.Value{Nil: &b}
	case aero.HLLValue:
		return &pb.Value{Hll: v.GetObject().([]byte)}
	case aero.GeoJSONValue:
		s := v.GetObject().(string)
		return &pb.Value{Geo: &s}
	}

	if in == nil {
		b := true
		return &pb.Value{Nil: &b}
	}

	log.Printf("%#v", in)
	log.Println(reflect.TypeOf(in).Elem())
	panic("UNREACHABLE")
}
