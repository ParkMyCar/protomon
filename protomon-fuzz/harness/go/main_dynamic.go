// Dynamic protobuf harness for protomon-fuzz.
//
// This harness can load any .proto file at runtime using protoc to generate
// a FileDescriptorSet, then uses dynamicpb to work with the messages.
//
// Usage:
//
//	# Encode text format to binary:
//	./harness_dynamic --mode=encode --proto=schema.proto --message=package.MessageName < input.textproto > output.bin
//
//	# Decode binary to text format:
//	./harness_dynamic --mode=decode --proto=schema.proto --message=package.MessageName < input.bin > output.textproto
//
//	# Roundtrip test:
//	./harness_dynamic --mode=roundtrip --proto=schema.proto --message=package.MessageName < input.textproto > output.bin
package main

import (
	"flag"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"google.golang.org/protobuf/encoding/prototext"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/reflect/protodesc"
	"google.golang.org/protobuf/reflect/protoreflect"
	"google.golang.org/protobuf/types/descriptorpb"
	"google.golang.org/protobuf/types/dynamicpb"
)

const maxInputSize = 100 * 1024 * 1024 // 100MB

var prettyTextOptions = prototext.MarshalOptions{
	Multiline: true,
	Indent:    "  ",
}

var (
	mode      = flag.String("mode", "encode", "Mode: 'encode' (text->binary), 'decode' (binary->text), or 'roundtrip'")
	protoFile = flag.String("proto", "", "Path to .proto file")
	message   = flag.String("message", "", "Fully qualified message name (e.g., package.MessageName)")
	protoPath = flag.String("proto_path", "", "Proto import path (defaults to directory containing proto file)")
)

func main() {
	flag.Parse()

	if *protoFile == "" {
		fmt.Fprintln(os.Stderr, "Error: --proto is required")
		os.Exit(1)
	}
	if *message == "" {
		fmt.Fprintln(os.Stderr, "Error: --message is required")
		os.Exit(1)
	}

	// Load the proto file and find the message descriptor
	msgDesc, err := loadMessageDescriptor(*protoFile, *message, *protoPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error loading proto: %v\n", err)
		os.Exit(1)
	}

	// Run the requested mode
	switch *mode {
	case "encode":
		err = encode(msgDesc)
	case "decode":
		err = decode(msgDesc)
	case "roundtrip":
		err = roundtrip(msgDesc)
	default:
		fmt.Fprintf(os.Stderr, "Unknown mode: %s\n", *mode)
		os.Exit(1)
	}

	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
}

// loadMessageDescriptor uses protoc to compile the proto file and returns the message descriptor.
func loadMessageDescriptor(protoFile, messageName, protoPath string) (protoreflect.MessageDescriptor, error) {
	// Get absolute path to proto file
	absProto, err := filepath.Abs(protoFile)
	if err != nil {
		return nil, fmt.Errorf("failed to get absolute path: %w", err)
	}

	// Default proto_path to the directory containing the proto file
	if protoPath == "" {
		protoPath = filepath.Dir(absProto)
	}

	// Create a temp file for the descriptor set
	tmpFile, err := os.CreateTemp("", "descriptor-*.pb")
	if err != nil {
		return nil, fmt.Errorf("failed to create temp file: %w", err)
	}
	tmpFile.Close()
	defer os.Remove(tmpFile.Name())

	// Run protoc to generate the descriptor set
	cmd := exec.Command("protoc",
		"--proto_path="+protoPath,
		"--descriptor_set_out="+tmpFile.Name(),
		"--include_imports",
		filepath.Base(absProto),
	)
	cmd.Dir = protoPath
	output, err := cmd.CombinedOutput()
	if err != nil {
		return nil, fmt.Errorf("protoc failed: %v\n%s", err, output)
	}

	// Read the descriptor set
	descBytes, err := os.ReadFile(tmpFile.Name())
	if err != nil {
		return nil, fmt.Errorf("failed to read descriptor: %w", err)
	}

	// Parse the FileDescriptorSet
	fdSet := &descriptorpb.FileDescriptorSet{}
	if err := proto.Unmarshal(descBytes, fdSet); err != nil {
		return nil, fmt.Errorf("failed to parse descriptor: %w", err)
	}

	// Build file descriptors and register them
	files, err := protodesc.NewFiles(fdSet)
	if err != nil {
		return nil, fmt.Errorf("failed to create file descriptors: %w", err)
	}

	// Find the message descriptor
	fullName := protoreflect.FullName(messageName)
	desc, err := files.FindDescriptorByName(fullName)
	if err != nil {
		// List available messages
		var available []string
		files.RangeFiles(func(fd protoreflect.FileDescriptor) bool {
			msgs := fd.Messages()
			for i := 0; i < msgs.Len(); i++ {
				available = append(available, string(msgs.Get(i).FullName()))
			}
			return true
		})
		return nil, fmt.Errorf("message not found: %s\nAvailable: %s", messageName, strings.Join(available, ", "))
	}

	msgDesc, ok := desc.(protoreflect.MessageDescriptor)
	if !ok {
		return nil, fmt.Errorf("%s is not a message type", messageName)
	}

	return msgDesc, nil
}

// readLimited reads from r with a size limit.
func readLimited(r io.Reader) ([]byte, error) {
	limitedReader := io.LimitReader(r, maxInputSize+1)
	data, err := io.ReadAll(limitedReader)
	if err != nil {
		return nil, err
	}
	if len(data) > maxInputSize {
		return nil, fmt.Errorf("input exceeds maximum size of %d bytes", maxInputSize)
	}
	return data, nil
}

// newMessage creates a new dynamic message from the descriptor.
func newMessage(desc protoreflect.MessageDescriptor) *dynamicpb.Message {
	return dynamicpb.NewMessage(desc)
}

func encode(msgDesc protoreflect.MessageDescriptor) error {
	textInput, err := readLimited(os.Stdin)
	if err != nil {
		return fmt.Errorf("failed to read stdin: %w", err)
	}

	msg := newMessage(msgDesc)
	if err := prototext.Unmarshal(textInput, msg); err != nil {
		return fmt.Errorf("failed to parse text format: %w", err)
	}

	binaryOutput, err := proto.Marshal(msg)
	if err != nil {
		return fmt.Errorf("failed to serialize: %w", err)
	}

	if _, err := os.Stdout.Write(binaryOutput); err != nil {
		return fmt.Errorf("failed to write output: %w", err)
	}

	return nil
}

func decode(msgDesc protoreflect.MessageDescriptor) error {
	binaryInput, err := readLimited(os.Stdin)
	if err != nil {
		return fmt.Errorf("failed to read stdin: %w", err)
	}

	msg := newMessage(msgDesc)
	if err := proto.Unmarshal(binaryInput, msg); err != nil {
		return fmt.Errorf("failed to parse binary: %w", err)
	}

	textOutput, err := prettyTextOptions.Marshal(msg)
	if err != nil {
		return fmt.Errorf("failed to print text format: %w", err)
	}

	fmt.Print(string(textOutput))
	return nil
}

func roundtrip(msgDesc protoreflect.MessageDescriptor) error {
	textInput, err := readLimited(os.Stdin)
	if err != nil {
		return fmt.Errorf("failed to read stdin: %w", err)
	}

	msg1 := newMessage(msgDesc)
	if err := prototext.Unmarshal(textInput, msg1); err != nil {
		return fmt.Errorf("failed to parse text format: %w", err)
	}

	binary, err := proto.Marshal(msg1)
	if err != nil {
		return fmt.Errorf("failed to serialize: %w", err)
	}

	msg2 := newMessage(msgDesc)
	if err := proto.Unmarshal(binary, msg2); err != nil {
		return fmt.Errorf("failed to parse binary: %w", err)
	}

	if !proto.Equal(msg1, msg2) {
		text1, err1 := prettyTextOptions.Marshal(msg1)
		text2, err2 := prettyTextOptions.Marshal(msg2)

		originalText := string(text1)
		if err1 != nil {
			originalText = fmt.Sprintf("<marshal error: %v>", err1)
		}

		roundtripText := string(text2)
		if err2 != nil {
			roundtripText = fmt.Sprintf("<marshal error: %v>", err2)
		}

		return fmt.Errorf("roundtrip mismatch!\nOriginal:\n%s\nAfter roundtrip:\n%s",
			originalText, roundtripText)
	}

	if _, err := os.Stdout.Write(binary); err != nil {
		return fmt.Errorf("failed to write output: %w", err)
	}

	fmt.Fprintf(os.Stderr, "Roundtrip OK (%d bytes)\n", len(binary))
	return nil
}

