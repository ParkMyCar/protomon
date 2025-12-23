// Go protobuf harness for protomon-fuzz.
//
// This tool encodes and decodes protobuf messages for cross-language testing.
//
// Usage:
//
//	# Encode text format to binary:
//	./harness -mode=encode < input.textproto > output.bin
//
//	# Decode binary to text format:
//	./harness -mode=decode < input.bin > output.textproto
//
//	# Roundtrip test (encode then decode, compare):
//	./harness -mode=roundtrip < input.textproto > output.bin
package main

import (
	"flag"
	"fmt"
	"io"
	"os"

	"google.golang.org/protobuf/encoding/prototext"
	"google.golang.org/protobuf/proto"

	pb "protomon-fuzz-harness/proto"
)

const maxInputSize = 100 * 1024 * 1024 // 100MB

// MessageFactory creates a new proto.Message instance.
type MessageFactory func() proto.Message

// Consistent text format options for pretty printing.
var prettyTextOptions = prototext.MarshalOptions{
	Multiline: true,
	Indent:    "  ",
}

var (
	mode        = flag.String("mode", "encode", "Mode: 'encode' (text->binary), 'decode' (binary->text), or 'roundtrip'")
	messageType = flag.String("message", "TestMessage", "Message type: 'TestMessage' or 'NestedExample'")
)

func main() {
	flag.Parse()

	var err error
	switch *messageType {
	case "TestMessage":
		err = run(*mode, func() proto.Message { return &pb.TestMessage{} })
	case "NestedExample":
		err = run(*mode, func() proto.Message { return &pb.NestedExample{} })
	default:
		fmt.Fprintf(os.Stderr, "Unknown message type: %s\n", *messageType)
		fmt.Fprintf(os.Stderr, "Available: TestMessage, NestedExample\n")
		os.Exit(1)
	}

	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
}

// readLimited reads from r with a size limit to prevent memory exhaustion.
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

func run(mode string, newMsg MessageFactory) error {
	switch mode {
	case "encode":
		return encode(newMsg)
	case "decode":
		return decode(newMsg)
	case "roundtrip":
		return roundtrip(newMsg)
	default:
		return fmt.Errorf("unknown mode: %s", mode)
	}
}

func encode(newMsg MessageFactory) error {
	// Read text format from stdin with size limit
	textInput, err := readLimited(os.Stdin)
	if err != nil {
		return fmt.Errorf("failed to read stdin: %w", err)
	}

	// Parse text format
	msg := newMsg()
	if err := prototext.Unmarshal(textInput, msg); err != nil {
		return fmt.Errorf("failed to parse text format: %w", err)
	}

	// Serialize to binary
	binaryOutput, err := proto.Marshal(msg)
	if err != nil {
		return fmt.Errorf("failed to serialize: %w", err)
	}

	// Write binary to stdout
	if _, err := os.Stdout.Write(binaryOutput); err != nil {
		return fmt.Errorf("failed to write output: %w", err)
	}

	return nil
}

func decode(newMsg MessageFactory) error {
	// Read binary from stdin with size limit
	binaryInput, err := readLimited(os.Stdin)
	if err != nil {
		return fmt.Errorf("failed to read stdin: %w", err)
	}

	// Parse binary format
	msg := newMsg()
	if err := proto.Unmarshal(binaryInput, msg); err != nil {
		return fmt.Errorf("failed to parse binary: %w", err)
	}

	// Print as text format using consistent options
	textOutput, err := prettyTextOptions.Marshal(msg)
	if err != nil {
		return fmt.Errorf("failed to print text format: %w", err)
	}

	fmt.Print(string(textOutput))
	return nil
}

func roundtrip(newMsg MessageFactory) error {
	// Read text format from stdin with size limit
	textInput, err := readLimited(os.Stdin)
	if err != nil {
		return fmt.Errorf("failed to read stdin: %w", err)
	}

	// Parse text format
	msg1 := newMsg()
	if err := prototext.Unmarshal(textInput, msg1); err != nil {
		return fmt.Errorf("failed to parse text format: %w", err)
	}

	// Serialize to binary
	binary, err := proto.Marshal(msg1)
	if err != nil {
		return fmt.Errorf("failed to serialize: %w", err)
	}

	// Parse binary back
	msg2 := newMsg()
	if err := proto.Unmarshal(binary, msg2); err != nil {
		return fmt.Errorf("failed to parse binary: %w", err)
	}

	// Compare
	if !proto.Equal(msg1, msg2) {
		// Handle potential marshal errors in error reporting
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

	// Output the binary
	if _, err := os.Stdout.Write(binary); err != nil {
		return fmt.Errorf("failed to write output: %w", err)
	}

	fmt.Fprintf(os.Stderr, "Roundtrip OK (%d bytes)\n", len(binary))
	return nil
}
