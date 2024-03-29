// Code generated by protoc-gen-go-grpc. DO NOT EDIT.
// versions:
// - protoc-gen-go-grpc v1.3.0
// - protoc             v3.12.4
// source: asld_kvs.proto

package proto

import (
	context "context"
	grpc "google.golang.org/grpc"
	codes "google.golang.org/grpc/codes"
	status "google.golang.org/grpc/status"
)

// This is a compile-time assertion to ensure that this generated file
// is compatible with the grpc package it is being compiled against.
// Requires gRPC-Go v1.32.0 or later.
const _ = grpc.SupportPackageIsVersion7

const (
	KVS_Put_FullMethodName          = "/com.aerospike.daemon.KVS/Put"
	KVS_Add_FullMethodName          = "/com.aerospike.daemon.KVS/Add"
	KVS_Append_FullMethodName       = "/com.aerospike.daemon.KVS/Append"
	KVS_Prepend_FullMethodName      = "/com.aerospike.daemon.KVS/Prepend"
	KVS_Get_FullMethodName          = "/com.aerospike.daemon.KVS/Get"
	KVS_GetHeader_FullMethodName    = "/com.aerospike.daemon.KVS/GetHeader"
	KVS_Exists_FullMethodName       = "/com.aerospike.daemon.KVS/Exists"
	KVS_Delete_FullMethodName       = "/com.aerospike.daemon.KVS/Delete"
	KVS_Touch_FullMethodName        = "/com.aerospike.daemon.KVS/Touch"
	KVS_BatchOperate_FullMethodName = "/com.aerospike.daemon.KVS/BatchOperate"
	KVS_CreateIndex_FullMethodName  = "/com.aerospike.daemon.KVS/CreateIndex"
	KVS_DropIndex_FullMethodName    = "/com.aerospike.daemon.KVS/DropIndex"
	KVS_Truncate_FullMethodName     = "/com.aerospike.daemon.KVS/Truncate"
)

// KVSClient is the client API for KVS service.
//
// For semantics around ctx use and closing/ending streaming RPCs, please refer to https://pkg.go.dev/google.golang.org/grpc/?tab=doc#ClientConn.NewStream.
type KVSClient interface {
	// Put a single record
	Put(ctx context.Context, in *AerospikePutRequest, opts ...grpc.CallOption) (*Error, error)
	// Add a single record
	Add(ctx context.Context, in *AerospikePutRequest, opts ...grpc.CallOption) (*Error, error)
	// Append a single record
	Append(ctx context.Context, in *AerospikePutRequest, opts ...grpc.CallOption) (*Error, error)
	// Prepend a single record
	Prepend(ctx context.Context, in *AerospikePutRequest, opts ...grpc.CallOption) (*Error, error)
	// Read a single record
	Get(ctx context.Context, in *AerospikeGetRequest, opts ...grpc.CallOption) (*AerospikeSingleResponse, error)
	// Get a single record header containing metadata like generation, expiration
	GetHeader(ctx context.Context, in *AerospikeGetHeaderRequest, opts ...grpc.CallOption) (*AerospikeSingleResponse, error)
	// Check if a record exists.
	Exists(ctx context.Context, in *AerospikeExistsRequest, opts ...grpc.CallOption) (*AerospikeExistsResponse, error)
	// Delete a single record.
	Delete(ctx context.Context, in *AerospikeDeleteRequest, opts ...grpc.CallOption) (*AerospikeDeleteResponse, error)
	// Reset single record's time to expiration using the write policy's expiration.
	Touch(ctx context.Context, in *AerospikeTouchRequest, opts ...grpc.CallOption) (*Error, error)
	// Process batch requests.
	BatchOperate(ctx context.Context, in *AerospikeBatchOperateRequest, opts ...grpc.CallOption) (*AerospikeBatchOperateResponse, error)
	// Process batch requests.
	CreateIndex(ctx context.Context, in *AerospikeCreateIndexRequest, opts ...grpc.CallOption) (*AerospikeCreateIndexResponse, error)
	// Process batch requests.
	DropIndex(ctx context.Context, in *AerospikeDropIndexRequest, opts ...grpc.CallOption) (*AerospikeDropIndexResponse, error)
	// Truncate removes records in specified namespace/set efficiently.
	Truncate(ctx context.Context, in *AerospikeTruncateRequest, opts ...grpc.CallOption) (*AerospikeTruncateResponse, error)
}

type kVSClient struct {
	cc grpc.ClientConnInterface
}

func NewKVSClient(cc grpc.ClientConnInterface) KVSClient {
	return &kVSClient{cc}
}

func (c *kVSClient) Put(ctx context.Context, in *AerospikePutRequest, opts ...grpc.CallOption) (*Error, error) {
	out := new(Error)
	err := c.cc.Invoke(ctx, KVS_Put_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *kVSClient) Add(ctx context.Context, in *AerospikePutRequest, opts ...grpc.CallOption) (*Error, error) {
	out := new(Error)
	err := c.cc.Invoke(ctx, KVS_Add_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *kVSClient) Append(ctx context.Context, in *AerospikePutRequest, opts ...grpc.CallOption) (*Error, error) {
	out := new(Error)
	err := c.cc.Invoke(ctx, KVS_Append_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *kVSClient) Prepend(ctx context.Context, in *AerospikePutRequest, opts ...grpc.CallOption) (*Error, error) {
	out := new(Error)
	err := c.cc.Invoke(ctx, KVS_Prepend_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *kVSClient) Get(ctx context.Context, in *AerospikeGetRequest, opts ...grpc.CallOption) (*AerospikeSingleResponse, error) {
	out := new(AerospikeSingleResponse)
	err := c.cc.Invoke(ctx, KVS_Get_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *kVSClient) GetHeader(ctx context.Context, in *AerospikeGetHeaderRequest, opts ...grpc.CallOption) (*AerospikeSingleResponse, error) {
	out := new(AerospikeSingleResponse)
	err := c.cc.Invoke(ctx, KVS_GetHeader_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *kVSClient) Exists(ctx context.Context, in *AerospikeExistsRequest, opts ...grpc.CallOption) (*AerospikeExistsResponse, error) {
	out := new(AerospikeExistsResponse)
	err := c.cc.Invoke(ctx, KVS_Exists_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *kVSClient) Delete(ctx context.Context, in *AerospikeDeleteRequest, opts ...grpc.CallOption) (*AerospikeDeleteResponse, error) {
	out := new(AerospikeDeleteResponse)
	err := c.cc.Invoke(ctx, KVS_Delete_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *kVSClient) Touch(ctx context.Context, in *AerospikeTouchRequest, opts ...grpc.CallOption) (*Error, error) {
	out := new(Error)
	err := c.cc.Invoke(ctx, KVS_Touch_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *kVSClient) BatchOperate(ctx context.Context, in *AerospikeBatchOperateRequest, opts ...grpc.CallOption) (*AerospikeBatchOperateResponse, error) {
	out := new(AerospikeBatchOperateResponse)
	err := c.cc.Invoke(ctx, KVS_BatchOperate_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *kVSClient) CreateIndex(ctx context.Context, in *AerospikeCreateIndexRequest, opts ...grpc.CallOption) (*AerospikeCreateIndexResponse, error) {
	out := new(AerospikeCreateIndexResponse)
	err := c.cc.Invoke(ctx, KVS_CreateIndex_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *kVSClient) DropIndex(ctx context.Context, in *AerospikeDropIndexRequest, opts ...grpc.CallOption) (*AerospikeDropIndexResponse, error) {
	out := new(AerospikeDropIndexResponse)
	err := c.cc.Invoke(ctx, KVS_DropIndex_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *kVSClient) Truncate(ctx context.Context, in *AerospikeTruncateRequest, opts ...grpc.CallOption) (*AerospikeTruncateResponse, error) {
	out := new(AerospikeTruncateResponse)
	err := c.cc.Invoke(ctx, KVS_Truncate_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

// KVSServer is the server API for KVS service.
// All implementations must embed UnimplementedKVSServer
// for forward compatibility
type KVSServer interface {
	// Put a single record
	Put(context.Context, *AerospikePutRequest) (*Error, error)
	// Add a single record
	Add(context.Context, *AerospikePutRequest) (*Error, error)
	// Append a single record
	Append(context.Context, *AerospikePutRequest) (*Error, error)
	// Prepend a single record
	Prepend(context.Context, *AerospikePutRequest) (*Error, error)
	// Read a single record
	Get(context.Context, *AerospikeGetRequest) (*AerospikeSingleResponse, error)
	// Get a single record header containing metadata like generation, expiration
	GetHeader(context.Context, *AerospikeGetHeaderRequest) (*AerospikeSingleResponse, error)
	// Check if a record exists.
	Exists(context.Context, *AerospikeExistsRequest) (*AerospikeExistsResponse, error)
	// Delete a single record.
	Delete(context.Context, *AerospikeDeleteRequest) (*AerospikeDeleteResponse, error)
	// Reset single record's time to expiration using the write policy's expiration.
	Touch(context.Context, *AerospikeTouchRequest) (*Error, error)
	// Process batch requests.
	BatchOperate(context.Context, *AerospikeBatchOperateRequest) (*AerospikeBatchOperateResponse, error)
	// Process batch requests.
	CreateIndex(context.Context, *AerospikeCreateIndexRequest) (*AerospikeCreateIndexResponse, error)
	// Process batch requests.
	DropIndex(context.Context, *AerospikeDropIndexRequest) (*AerospikeDropIndexResponse, error)
	// Truncate removes records in specified namespace/set efficiently.
	Truncate(context.Context, *AerospikeTruncateRequest) (*AerospikeTruncateResponse, error)
	mustEmbedUnimplementedKVSServer()
}

// UnimplementedKVSServer must be embedded to have forward compatible implementations.
type UnimplementedKVSServer struct {
}

func (UnimplementedKVSServer) Put(context.Context, *AerospikePutRequest) (*Error, error) {
	return nil, status.Errorf(codes.Unimplemented, "method Put not implemented")
}
func (UnimplementedKVSServer) Add(context.Context, *AerospikePutRequest) (*Error, error) {
	return nil, status.Errorf(codes.Unimplemented, "method Add not implemented")
}
func (UnimplementedKVSServer) Append(context.Context, *AerospikePutRequest) (*Error, error) {
	return nil, status.Errorf(codes.Unimplemented, "method Append not implemented")
}
func (UnimplementedKVSServer) Prepend(context.Context, *AerospikePutRequest) (*Error, error) {
	return nil, status.Errorf(codes.Unimplemented, "method Prepend not implemented")
}
func (UnimplementedKVSServer) Get(context.Context, *AerospikeGetRequest) (*AerospikeSingleResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method Get not implemented")
}
func (UnimplementedKVSServer) GetHeader(context.Context, *AerospikeGetHeaderRequest) (*AerospikeSingleResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method GetHeader not implemented")
}
func (UnimplementedKVSServer) Exists(context.Context, *AerospikeExistsRequest) (*AerospikeExistsResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method Exists not implemented")
}
func (UnimplementedKVSServer) Delete(context.Context, *AerospikeDeleteRequest) (*AerospikeDeleteResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method Delete not implemented")
}
func (UnimplementedKVSServer) Touch(context.Context, *AerospikeTouchRequest) (*Error, error) {
	return nil, status.Errorf(codes.Unimplemented, "method Touch not implemented")
}
func (UnimplementedKVSServer) BatchOperate(context.Context, *AerospikeBatchOperateRequest) (*AerospikeBatchOperateResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method BatchOperate not implemented")
}
func (UnimplementedKVSServer) CreateIndex(context.Context, *AerospikeCreateIndexRequest) (*AerospikeCreateIndexResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method CreateIndex not implemented")
}
func (UnimplementedKVSServer) DropIndex(context.Context, *AerospikeDropIndexRequest) (*AerospikeDropIndexResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method DropIndex not implemented")
}
func (UnimplementedKVSServer) Truncate(context.Context, *AerospikeTruncateRequest) (*AerospikeTruncateResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method Truncate not implemented")
}
func (UnimplementedKVSServer) mustEmbedUnimplementedKVSServer() {}

// UnsafeKVSServer may be embedded to opt out of forward compatibility for this service.
// Use of this interface is not recommended, as added methods to KVSServer will
// result in compilation errors.
type UnsafeKVSServer interface {
	mustEmbedUnimplementedKVSServer()
}

func RegisterKVSServer(s grpc.ServiceRegistrar, srv KVSServer) {
	s.RegisterService(&KVS_ServiceDesc, srv)
}

func _KVS_Put_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AerospikePutRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(KVSServer).Put(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: KVS_Put_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(KVSServer).Put(ctx, req.(*AerospikePutRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _KVS_Add_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AerospikePutRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(KVSServer).Add(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: KVS_Add_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(KVSServer).Add(ctx, req.(*AerospikePutRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _KVS_Append_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AerospikePutRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(KVSServer).Append(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: KVS_Append_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(KVSServer).Append(ctx, req.(*AerospikePutRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _KVS_Prepend_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AerospikePutRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(KVSServer).Prepend(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: KVS_Prepend_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(KVSServer).Prepend(ctx, req.(*AerospikePutRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _KVS_Get_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AerospikeGetRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(KVSServer).Get(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: KVS_Get_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(KVSServer).Get(ctx, req.(*AerospikeGetRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _KVS_GetHeader_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AerospikeGetHeaderRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(KVSServer).GetHeader(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: KVS_GetHeader_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(KVSServer).GetHeader(ctx, req.(*AerospikeGetHeaderRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _KVS_Exists_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AerospikeExistsRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(KVSServer).Exists(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: KVS_Exists_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(KVSServer).Exists(ctx, req.(*AerospikeExistsRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _KVS_Delete_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AerospikeDeleteRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(KVSServer).Delete(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: KVS_Delete_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(KVSServer).Delete(ctx, req.(*AerospikeDeleteRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _KVS_Touch_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AerospikeTouchRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(KVSServer).Touch(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: KVS_Touch_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(KVSServer).Touch(ctx, req.(*AerospikeTouchRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _KVS_BatchOperate_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AerospikeBatchOperateRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(KVSServer).BatchOperate(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: KVS_BatchOperate_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(KVSServer).BatchOperate(ctx, req.(*AerospikeBatchOperateRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _KVS_CreateIndex_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AerospikeCreateIndexRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(KVSServer).CreateIndex(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: KVS_CreateIndex_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(KVSServer).CreateIndex(ctx, req.(*AerospikeCreateIndexRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _KVS_DropIndex_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AerospikeDropIndexRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(KVSServer).DropIndex(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: KVS_DropIndex_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(KVSServer).DropIndex(ctx, req.(*AerospikeDropIndexRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _KVS_Truncate_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AerospikeTruncateRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(KVSServer).Truncate(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: KVS_Truncate_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(KVSServer).Truncate(ctx, req.(*AerospikeTruncateRequest))
	}
	return interceptor(ctx, in, info, handler)
}

// KVS_ServiceDesc is the grpc.ServiceDesc for KVS service.
// It's only intended for direct use with grpc.RegisterService,
// and not to be introspected or modified (even as a copy)
var KVS_ServiceDesc = grpc.ServiceDesc{
	ServiceName: "com.aerospike.daemon.KVS",
	HandlerType: (*KVSServer)(nil),
	Methods: []grpc.MethodDesc{
		{
			MethodName: "Put",
			Handler:    _KVS_Put_Handler,
		},
		{
			MethodName: "Add",
			Handler:    _KVS_Add_Handler,
		},
		{
			MethodName: "Append",
			Handler:    _KVS_Append_Handler,
		},
		{
			MethodName: "Prepend",
			Handler:    _KVS_Prepend_Handler,
		},
		{
			MethodName: "Get",
			Handler:    _KVS_Get_Handler,
		},
		{
			MethodName: "GetHeader",
			Handler:    _KVS_GetHeader_Handler,
		},
		{
			MethodName: "Exists",
			Handler:    _KVS_Exists_Handler,
		},
		{
			MethodName: "Delete",
			Handler:    _KVS_Delete_Handler,
		},
		{
			MethodName: "Touch",
			Handler:    _KVS_Touch_Handler,
		},
		{
			MethodName: "BatchOperate",
			Handler:    _KVS_BatchOperate_Handler,
		},
		{
			MethodName: "CreateIndex",
			Handler:    _KVS_CreateIndex_Handler,
		},
		{
			MethodName: "DropIndex",
			Handler:    _KVS_DropIndex_Handler,
		},
		{
			MethodName: "Truncate",
			Handler:    _KVS_Truncate_Handler,
		},
	},
	Streams:  []grpc.StreamDesc{},
	Metadata: "asld_kvs.proto",
}
