package bridge

// #cgo LDFLAGS: -lestuary_bridge -lm -ldl -L ../target/debug/
/*
#include "bindings.h"
*/
import "C"
import (
	"bufio"
	"errors"
	"sync"
	"unsafe"

	"go.gazette.dev/core/broker/protocol"
	"go.gazette.dev/core/message"
)

// JSONPointer wraps the native est_json_ptr_t.
type JSONPointer struct {
	wrapped *C.est_json_ptr_t
}

// NewJSONPointer constructs a new, parsed JSONPointer.
func NewJSONPointer(ptr string) (JSONPointer, error) {
	var ptrC = C.CString(ptr)
	defer C.free(unsafe.Pointer(ptrC))

	var out *C.est_json_ptr_t
	if s := C.est_json_ptr_new(ptrC, &out); s != C.EST_OK {
		return JSONPointer{}, statusError(s)
	}
	return JSONPointer{wrapped: out}, nil
}

// MustJSONPointer parses the given JSONPointer and panics if an error occurs.
func MustJSONPointer(ptr string) JSONPointer {
	if o, err := NewJSONPointer(ptr); err != nil {
		panic(err)
	} else {
		return o
	}
}

// Drop the JSONPointer, releasing underlying resources.
func (p JSONPointer) Drop() {
	C.est_json_ptr_drop(p.wrapped)
}

// Message wraps the native est_msg_t.
type Message struct {
	wrapped *C.est_msg_t
}

// NewMessage returns a new, empty Message with the given pointer.
func NewMessage(p JSONPointer) Message {
	return Message{wrapped: C.est_msg_new(p.wrapped)}
}

// GetUUID returns the UUID of the Message.
func (m Message) GetUUID() message.UUID {
	var uuidC = C.est_msg_get_uuid(m.wrapped)
	// Note RFC 4122 defines that UUIDs are always network byte order (big endian).
	return message.UUID(*(*[16]byte)((unsafe.Pointer)(&uuidC.bytes[0])))
}

// SetUUID updates the UUID of the Message.
func (m Message) SetUUID(uuid message.UUID) {
	var uuidC C.est_uuid_t
	*(*[16]byte)((unsafe.Pointer)(&uuidC.bytes[0])) = uuid
	C.est_msg_set_uuid(m.wrapped, uuidC)
}

// NewAcknowledgement returns a JSON document with an initialized UUID
// location, but which is otherwise empty.
func (m Message) NewAcknowledgement(protocol.Journal) message.Message {
	return Message{wrapped: C.est_msg_new_acknowledgement(m.wrapped)}
}

// FieldVisitor visits ordered field locations of a Message's JSON document.
// It's used with the VisitFields method.
type FieldVisitor interface {
	VisitDoesNotExist(index int)
	VisitNull(index int)
	VisitBool(index int, value bool)
	VisitUnsigned(index int, value uint64)
	VisitSigned(index int, value int64)
	VisitFloat(index int, value float64)
	VisitString(index int, value []byte)
	VisitObject(index int, encoded []byte)
	VisitArray(index int, encoded []byte)
}

// VisitFields invokes the FieldVisitor for each ordered JSONPointer.
func (m Message) VisitFields(fv FieldVisitor, ptrs ...JSONPointer) {
	if len(ptrs) == 0 {
		return
	}

	var fields = make([]C.est_extract_field_t, len(ptrs))
	for i, ptr := range ptrs {
		fields[i].ptr = ptr.wrapped
	}

	var buf = bufferPool.Get().([]byte)
	for {
		var l = C.est_msg_extract_fields(m.wrapped,
			&fields[0], (C.uintptr_t)(len(fields)),
			(*C.uint8_t)(&buf[0]), (C.uintptr_t)(len(buf)))

		if int(l) < len(buf) {
			break
		} else {
			buf = make([]byte, roundUp(int(l)))
			continue
		}
	}

	for i, field := range fields {
		switch field.type_ {
		case C.EST_DOES_NOT_EXIST:
			fv.VisitDoesNotExist(i)
		case C.EST_NULL:
			fv.VisitNull(i)
		case C.EST_TRUE:
			fv.VisitBool(i, true)
		case C.EST_FALSE:
			fv.VisitBool(i, false)
		case C.EST_UNSIGNED:
			fv.VisitUnsigned(i, uint64(field.unsigned_))
		case C.EST_SIGNED:
			fv.VisitSigned(i, int64(field.signed_))
		case C.EST_FLOAT:
			fv.VisitFloat(i, float64(field.float_))
		case C.EST_STRING:
			fv.VisitString(i, buf[field.begin:field.end])
		case C.EST_OBJECT:
			fv.VisitObject(i, buf[field.begin:field.end])
		case C.EST_ARRAY:
			fv.VisitArray(i, buf[field.begin:field.end])
		}
	}
	bufferPool.Put(buf)
}

// HashFields produces a combined, stable, deep hash of the values at
// the given document locations.
func (m Message) HashFields(ptrs ...JSONPointer) uint64 {
	if len(ptrs) == 0 {
		return 0
	}
	var hash = C.est_msg_hash_fields(m.wrapped,
		(**C.est_json_ptr_t)((unsafe.Pointer)(&ptrs[0])), (C.uintptr_t)(len(ptrs)))
	return uint64(hash)
}

// MarshalJSONTo marshals the JSON message to the Writer.
func (m Message) MarshalJSONTo(b *bufio.Writer) (int, error) {
	var buf = bufferPool.Get().([]byte)
	for {
		var l = C.est_msg_marshal_json(m.wrapped,
			(*C.uint8_t)(&buf[0]), (C.uintptr_t)(len(buf)))

		if int(l) < len(buf) {
			var n, err = b.Write(buf[:l])
			bufferPool.Put(buf)
			return n, err
		}
		buf = make([]byte, roundUp(int(l)))
	}
}

// UnmarshalJSON unmarshals the Message from JSON.
func (m Message) UnmarshalJSON(buf []byte) error {
	var status = C.est_msg_unmarshal_json(m.wrapped,
		(*C.uint8_t)(&buf[0]), (C.uintptr_t)(len(buf)))

	if status != C.EST_OK {
		return statusError(status)
	}
	return nil
}

// Drop the Message, releasing underlying resources.
func (m Message) Drop() {
	C.est_msg_drop(m.wrapped)
}

func roundUp(n int) int {
	var count = 0
	for n != 0 {
		n = n >> 1
		count++
	}
	return 1 << count
}

var bufferPool = sync.Pool{
	New: func() interface{} { return make([]byte, 1024) },
}

func statusError(s C.est_status_t) error {
	var buf [64]byte
	var l = C.est_status_description(s, (*C.uint8_t)(&buf[0]), (C.uintptr_t)(len(buf)))
	return errors.New(string(buf[:l]))
}
