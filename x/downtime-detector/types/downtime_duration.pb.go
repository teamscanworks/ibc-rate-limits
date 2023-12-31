// Code generated by protoc-gen-gogo. DO NOT EDIT.
// source: osmosis/downtime-detector/v1beta1/downtime_duration.proto

package types

import (
	fmt "fmt"
	_ "github.com/cosmos/cosmos-proto"
	_ "github.com/cosmos/cosmos-sdk/codec/types"
	_ "github.com/gogo/protobuf/gogoproto"
	proto "github.com/gogo/protobuf/proto"
	_ "github.com/gogo/protobuf/types"
	math "math"
)

// Reference imports to suppress errors if they are not otherwise used.
var _ = proto.Marshal
var _ = fmt.Errorf
var _ = math.Inf

// This is a compile-time assertion to ensure that this generated file
// is compatible with the proto package it is being compiled against.
// A compilation error at this line likely means your copy of the
// proto package needs to be updated.
const _ = proto.GoGoProtoPackageIsVersion3 // please upgrade the proto package

type Downtime int32

const (
	Downtime_DURATION_30S  Downtime = 0
	Downtime_DURATION_1M   Downtime = 1
	Downtime_DURATION_2M   Downtime = 2
	Downtime_DURATION_3M   Downtime = 3
	Downtime_DURATION_4M   Downtime = 4
	Downtime_DURATION_5M   Downtime = 5
	Downtime_DURATION_10M  Downtime = 6
	Downtime_DURATION_20M  Downtime = 7
	Downtime_DURATION_30M  Downtime = 8
	Downtime_DURATION_40M  Downtime = 9
	Downtime_DURATION_50M  Downtime = 10
	Downtime_DURATION_1H   Downtime = 11
	Downtime_DURATION_1_5H Downtime = 12
	Downtime_DURATION_2H   Downtime = 13
	Downtime_DURATION_2_5H Downtime = 14
	Downtime_DURATION_3H   Downtime = 15
	Downtime_DURATION_4H   Downtime = 16
	Downtime_DURATION_5H   Downtime = 17
	Downtime_DURATION_6H   Downtime = 18
	Downtime_DURATION_9H   Downtime = 19
	Downtime_DURATION_12H  Downtime = 20
	Downtime_DURATION_18H  Downtime = 21
	Downtime_DURATION_24H  Downtime = 22
	Downtime_DURATION_36H  Downtime = 23
	Downtime_DURATION_48H  Downtime = 24
)

var Downtime_name = map[int32]string{
	0:  "DURATION_30S",
	1:  "DURATION_1M",
	2:  "DURATION_2M",
	3:  "DURATION_3M",
	4:  "DURATION_4M",
	5:  "DURATION_5M",
	6:  "DURATION_10M",
	7:  "DURATION_20M",
	8:  "DURATION_30M",
	9:  "DURATION_40M",
	10: "DURATION_50M",
	11: "DURATION_1H",
	12: "DURATION_1_5H",
	13: "DURATION_2H",
	14: "DURATION_2_5H",
	15: "DURATION_3H",
	16: "DURATION_4H",
	17: "DURATION_5H",
	18: "DURATION_6H",
	19: "DURATION_9H",
	20: "DURATION_12H",
	21: "DURATION_18H",
	22: "DURATION_24H",
	23: "DURATION_36H",
	24: "DURATION_48H",
}

var Downtime_value = map[string]int32{
	"DURATION_30S":  0,
	"DURATION_1M":   1,
	"DURATION_2M":   2,
	"DURATION_3M":   3,
	"DURATION_4M":   4,
	"DURATION_5M":   5,
	"DURATION_10M":  6,
	"DURATION_20M":  7,
	"DURATION_30M":  8,
	"DURATION_40M":  9,
	"DURATION_50M":  10,
	"DURATION_1H":   11,
	"DURATION_1_5H": 12,
	"DURATION_2H":   13,
	"DURATION_2_5H": 14,
	"DURATION_3H":   15,
	"DURATION_4H":   16,
	"DURATION_5H":   17,
	"DURATION_6H":   18,
	"DURATION_9H":   19,
	"DURATION_12H":  20,
	"DURATION_18H":  21,
	"DURATION_24H":  22,
	"DURATION_36H":  23,
	"DURATION_48H":  24,
}

func (x Downtime) String() string {
	return proto.EnumName(Downtime_name, int32(x))
}

func (Downtime) EnumDescriptor() ([]byte, []int) {
	return fileDescriptor_21a1969f22fb2a7e, []int{0}
}

func init() {
	proto.RegisterEnum("osmosis.downtimedetector.v1beta1.Downtime", Downtime_name, Downtime_value)
}

func init() {
	proto.RegisterFile("osmosis/downtime-detector/v1beta1/downtime_duration.proto", fileDescriptor_21a1969f22fb2a7e)
}

var fileDescriptor_21a1969f22fb2a7e = []byte{
	// 386 bytes of a gzipped FileDescriptorProto
	0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff, 0x6c, 0x92, 0xbb, 0x6e, 0xe2, 0x50,
	0x10, 0x86, 0xed, 0x65, 0x97, 0x65, 0x0d, 0x2c, 0x83, 0x97, 0xbd, 0x51, 0x38, 0xa9, 0x23, 0xe1,
	0x63, 0x9b, 0x4b, 0xa0, 0x48, 0x91, 0x88, 0xe2, 0xa4, 0x38, 0x89, 0x94, 0x8b, 0x22, 0xa5, 0xb1,
	0x6c, 0x70, 0x1c, 0x4b, 0x98, 0x83, 0xf0, 0x81, 0x84, 0xb7, 0xc8, 0x33, 0xa5, 0x4a, 0x49, 0x99,
	0x32, 0x82, 0x17, 0x89, 0xf0, 0x85, 0x68, 0x50, 0x3a, 0xcf, 0x37, 0xf3, 0xfb, 0xfc, 0xff, 0x68,
	0x94, 0x1e, 0x8f, 0x42, 0x1e, 0x05, 0x11, 0x19, 0xf2, 0x87, 0xb1, 0x08, 0x42, 0xaf, 0x31, 0xf4,
	0x84, 0x37, 0x10, 0x7c, 0x4a, 0xe6, 0xa6, 0xeb, 0x09, 0xc7, 0xdc, 0x76, 0xec, 0xe1, 0x6c, 0xea,
	0x88, 0x80, 0x8f, 0xf5, 0xc9, 0x94, 0x0b, 0xae, 0xee, 0xa7, 0x52, 0x3d, 0x1b, 0xc8, 0x94, 0x7a,
	0xaa, 0xac, 0xd7, 0x7c, 0xee, 0xf3, 0x78, 0x98, 0x6c, 0xbe, 0x12, 0x5d, 0xfd, 0xbf, 0xcf, 0xb9,
	0x3f, 0xf2, 0x48, 0x5c, 0xb9, 0xb3, 0x3b, 0xe2, 0x8c, 0x17, 0x59, 0x6b, 0x10, 0xff, 0xd3, 0x4e,
	0x34, 0x49, 0x91, 0xb6, 0xb4, 0x5d, 0x15, 0x76, 0x53, 0xdf, 0xdb, 0xed, 0x6f, 0x1c, 0x45, 0xc2,
	0x09, 0x27, 0xc9, 0xc0, 0xc1, 0x73, 0x4e, 0x29, 0xf4, 0x53, 0xa7, 0x2a, 0x28, 0xa5, 0xfe, 0xf5,
	0xc5, 0xf1, 0xd5, 0xe9, 0xf9, 0x99, 0xdd, 0x34, 0x2e, 0x41, 0x52, 0x2b, 0x4a, 0x71, 0x4b, 0x4c,
	0x06, 0x32, 0x02, 0x16, 0x83, 0x2f, 0x08, 0x34, 0x19, 0xe4, 0x10, 0x68, 0x31, 0xf8, 0x8a, 0x40,
	0x9b, 0xc1, 0x37, 0xf4, 0x8c, 0x69, 0x30, 0xc8, 0x23, 0x62, 0x19, 0x0c, 0xbe, 0xef, 0x58, 0x61,
	0x50, 0x40, 0xa4, 0x65, 0x30, 0xf8, 0x81, 0x48, 0xdb, 0x60, 0xa0, 0x60, 0xbb, 0x14, 0x8a, 0x6a,
	0x55, 0x29, 0x7f, 0x00, 0xbb, 0x4d, 0xa1, 0x84, 0x13, 0x50, 0x28, 0xa3, 0x19, 0x6b, 0x33, 0xf3,
	0x13, 0x87, 0xa2, 0x50, 0xc1, 0xa1, 0x28, 0x00, 0x0e, 0x45, 0xa1, 0x8a, 0x40, 0x87, 0x82, 0x8a,
	0x40, 0x8f, 0xc2, 0x2f, 0x1c, 0xdb, 0xa2, 0x50, 0xc3, 0xa4, 0x4b, 0xe1, 0x37, 0x5e, 0x44, 0x8b,
	0xc2, 0x1f, 0xbc, 0x88, 0x0e, 0x85, 0xbf, 0x78, 0x11, 0x5d, 0x0a, 0xff, 0x4e, 0x6e, 0x5e, 0x56,
	0x9a, 0xbc, 0x5c, 0x69, 0xf2, 0xdb, 0x4a, 0x93, 0x9f, 0xd6, 0x9a, 0xb4, 0x5c, 0x6b, 0xd2, 0xeb,
	0x5a, 0x93, 0x6e, 0x8f, 0xfc, 0x40, 0xdc, 0xcf, 0x5c, 0x7d, 0xc0, 0x43, 0x92, 0x1e, 0x66, 0x63,
	0xe4, 0xb8, 0x51, 0x56, 0x90, 0xb9, 0x79, 0x48, 0x1e, 0x3f, 0x39, 0x73, 0xb1, 0x98, 0x78, 0x91,
	0x9b, 0x8f, 0x8f, 0xa4, 0xf9, 0x1e, 0x00, 0x00, 0xff, 0xff, 0xea, 0x78, 0xa7, 0x27, 0x10, 0x03,
	0x00, 0x00,
}
