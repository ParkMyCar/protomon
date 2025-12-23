// Dynamic protobuf message harness for protomon-fuzz.
//
// This tool uses protobuf's dynamic message API to work with any schema
// at runtime, without needing compile-time code generation.
//
// Usage:
//   # Encode text format to binary:
//   ./harness --mode=encode --proto=schema.proto --message=package.MessageName < input.textproto > output.bin
//
//   # Decode binary to text format:
//   ./harness --mode=decode --proto=schema.proto --message=package.MessageName < input.bin > output.textproto
//
//   # Roundtrip test (encode then decode, compare):
//   ./harness --mode=roundtrip --proto=schema.proto --message=package.MessageName < input.textproto

#include <fcntl.h>
#include <unistd.h>

#include <cerrno>
#include <cstring>
#include <fstream>
#include <iostream>
#include <sstream>
#include <string>

#ifdef _WIN32
#include <io.h>
#endif

#include "absl/flags/flag.h"
#include "absl/flags/parse.h"
#include "absl/strings/str_cat.h"
#include "google/protobuf/compiler/importer.h"
#include "google/protobuf/descriptor.h"
#include "google/protobuf/dynamic_message.h"
#include "google/protobuf/io/zero_copy_stream_impl.h"
#include "google/protobuf/text_format.h"
#include "google/protobuf/util/message_differencer.h"

ABSL_FLAG(std::string, mode, "encode",
          "Mode: 'encode' (text->binary), 'decode' (binary->text), or 'roundtrip'");
ABSL_FLAG(std::string, proto, "", "Path to .proto file");
ABSL_FLAG(std::string, message, "", "Fully qualified message name (e.g., package.MessageName)");
ABSL_FLAG(std::string, proto_path, ".", "Proto import path");

namespace {

constexpr size_t kReadBufferSize = 4096;
constexpr size_t kMaxInputSize = 100 * 1024 * 1024;  // 100MB

// Simple error collector that prints to stderr
class ErrorCollector : public google::protobuf::compiler::MultiFileErrorCollector {
 public:
  void RecordError(absl::string_view filename, int line, int column,
                   absl::string_view message) override {
    std::cerr << filename << ":" << line << ":" << column << ": " << message << std::endl;
  }

  void RecordWarning(absl::string_view filename, int line, int column,
                     absl::string_view message) override {
    std::cerr << "warning: " << filename << ":" << line << ":" << column << ": " << message
              << std::endl;
  }
};

std::string ReadAllFromFd(int fd) {
  std::string result;
  result.reserve(8192);  // Pre-allocate reasonable size

  char buffer[kReadBufferSize];
  ssize_t bytes_read;

  while ((bytes_read = read(fd, buffer, sizeof(buffer))) > 0) {
    result.append(buffer, bytes_read);

    if (result.size() > kMaxInputSize) {
      std::cerr << "Error: input exceeds maximum size of "
                << kMaxInputSize << " bytes" << std::endl;
      exit(1);
    }
  }

  if (bytes_read < 0) {
    std::cerr << "Error reading from file descriptor: "
              << strerror(errno) << std::endl;
    exit(1);
  }

  return result;
}

int Encode(const google::protobuf::Descriptor* descriptor,
           google::protobuf::DynamicMessageFactory* factory) {
  // Read text format from stdin
  std::string text_input = ReadAllFromFd(STDIN_FILENO);

  // Create a message instance
  const google::protobuf::Message* prototype = factory->GetPrototype(descriptor);
  if (prototype == nullptr) {
    std::cerr << "Failed to get prototype for message type" << std::endl;
    return 1;
  }
  std::unique_ptr<google::protobuf::Message> message(prototype->New());

  // Parse text format
  if (!google::protobuf::TextFormat::ParseFromString(text_input, message.get())) {
    std::cerr << "Failed to parse text format input" << std::endl;
    return 1;
  }

  // Serialize to binary and write to stdout
  std::string binary_output;
  if (!message->SerializeToString(&binary_output)) {
    std::cerr << "Failed to serialize message" << std::endl;
    return 1;
  }

  // Write binary to stdout
  std::cout.write(binary_output.data(), binary_output.size());
  return 0;
}

int Decode(const google::protobuf::Descriptor* descriptor,
           google::protobuf::DynamicMessageFactory* factory) {
  // Read binary from stdin
  std::string binary_input = ReadAllFromFd(STDIN_FILENO);

  // Create a message instance
  const google::protobuf::Message* prototype = factory->GetPrototype(descriptor);
  if (prototype == nullptr) {
    std::cerr << "Failed to get prototype for message type" << std::endl;
    return 1;
  }
  std::unique_ptr<google::protobuf::Message> message(prototype->New());

  // Parse binary format
  if (!message->ParseFromString(binary_input)) {
    std::cerr << "Failed to parse binary input" << std::endl;
    return 1;
  }

  // Print as text format
  std::string text_output;
  if (!google::protobuf::TextFormat::PrintToString(*message, &text_output)) {
    std::cerr << "Failed to print text format" << std::endl;
    return 1;
  }

  std::cout << text_output;
  return 0;
}

int Roundtrip(const google::protobuf::Descriptor* descriptor,
              google::protobuf::DynamicMessageFactory* factory) {
  // Read text format from stdin
  std::string text_input = ReadAllFromFd(STDIN_FILENO);

  // Create message instances
  const google::protobuf::Message* prototype = factory->GetPrototype(descriptor);
  if (prototype == nullptr) {
    std::cerr << "Failed to get prototype for message type" << std::endl;
    return 1;
  }
  std::unique_ptr<google::protobuf::Message> message1(prototype->New());
  std::unique_ptr<google::protobuf::Message> message2(prototype->New());

  // Parse text format
  if (!google::protobuf::TextFormat::ParseFromString(text_input, message1.get())) {
    std::cerr << "Failed to parse text format input" << std::endl;
    return 1;
  }

  // Serialize to binary
  std::string binary;
  if (!message1->SerializeToString(&binary)) {
    std::cerr << "Failed to serialize message" << std::endl;
    return 1;
  }

  // Parse binary back
  if (!message2->ParseFromString(binary)) {
    std::cerr << "Failed to parse binary" << std::endl;
    return 1;
  }

  // Compare using MessageDifferencer for canonical equality
  if (!google::protobuf::util::MessageDifferencer::Equals(*message1, *message2)) {
    std::cerr << "Roundtrip mismatch!" << std::endl;
    std::cerr << "Original:\n" << message1->DebugString() << std::endl;
    std::cerr << "After roundtrip:\n" << message2->DebugString() << std::endl;
    return 1;
  }

  // Output the binary (for use by other tools)
  std::cout.write(binary.data(), binary.size());

  std::cerr << "Roundtrip OK (" << binary.size() << " bytes)" << std::endl;
  return 0;
}

}  // namespace

int main(int argc, char* argv[]) {
  // Ensure binary mode for stdin/stdout on Windows
#ifdef _WIN32
  _setmode(_fileno(stdin), _O_BINARY);
  _setmode(_fileno(stdout), _O_BINARY);
#endif

  absl::ParseCommandLine(argc, argv);

  std::string mode = absl::GetFlag(FLAGS_mode);
  std::string proto_file = absl::GetFlag(FLAGS_proto);
  std::string message_name = absl::GetFlag(FLAGS_message);
  std::string proto_path = absl::GetFlag(FLAGS_proto_path);

  if (proto_file.empty()) {
    std::cerr << "Error: --proto is required" << std::endl;
    return 1;
  }
  if (message_name.empty()) {
    std::cerr << "Error: --message is required" << std::endl;
    return 1;
  }

  // Set up the proto importer
  google::protobuf::compiler::DiskSourceTree source_tree;
  source_tree.MapPath("", proto_path);

  ErrorCollector error_collector;
  google::protobuf::compiler::Importer importer(&source_tree, &error_collector);

  // Import the proto file
  const google::protobuf::FileDescriptor* file_desc = importer.Import(proto_file);
  if (file_desc == nullptr) {
    std::cerr << "Failed to import proto file: " << proto_file << std::endl;
    return 1;
  }

  // Find the message descriptor
  const google::protobuf::Descriptor* descriptor =
      importer.pool()->FindMessageTypeByName(message_name);
  if (descriptor == nullptr) {
    std::cerr << "Message not found: " << message_name << std::endl;
    std::cerr << "Available messages in " << proto_file << ":" << std::endl;
    for (int i = 0; i < file_desc->message_type_count(); i++) {
      std::cerr << "  " << file_desc->message_type(i)->full_name() << std::endl;
    }
    return 1;
  }

  // Create a dynamic message factory
  google::protobuf::DynamicMessageFactory factory;

  if (mode == "encode") {
    return Encode(descriptor, &factory);
  } else if (mode == "decode") {
    return Decode(descriptor, &factory);
  } else if (mode == "roundtrip") {
    return Roundtrip(descriptor, &factory);
  } else {
    std::cerr << "Unknown mode: " << mode << std::endl;
    return 1;
  }
}
