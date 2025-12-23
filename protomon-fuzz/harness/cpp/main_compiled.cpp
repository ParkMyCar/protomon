// Compiled protobuf harness for protomon-fuzz.
//
// This is a simpler harness that works with compile-time generated proto code.
// Use this when you have a fixed schema and want faster performance.
//
// Usage:
//   ./harness_compiled --mode=encode < input.textproto > output.bin
//   ./harness_compiled --mode=decode < input.bin > output.textproto
//   ./harness_compiled --mode=roundtrip < input.textproto > output.bin

#include <fcntl.h>
#include <unistd.h>

#include <cerrno>
#include <cstring>
#include <iostream>
#include <string>

#ifdef _WIN32
#include <io.h>
#endif

#include "absl/flags/flag.h"
#include "absl/flags/parse.h"
#include "google/protobuf/text_format.h"
#include "google/protobuf/util/message_differencer.h"
#include "proto/test.pb.h"

ABSL_FLAG(std::string, mode, "encode",
          "Mode: 'encode' (text->binary), 'decode' (binary->text), or 'roundtrip'");
ABSL_FLAG(std::string, message, "TestMessage",
          "Message type: 'TestMessage' or 'NestedExample'");

namespace {

constexpr size_t kReadBufferSize = 4096;
constexpr size_t kMaxInputSize = 100 * 1024 * 1024;  // 100MB

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

template <typename T>
int Encode() {
  std::string text_input = ReadAllFromFd(STDIN_FILENO);

  T message;
  if (!google::protobuf::TextFormat::ParseFromString(text_input, &message)) {
    std::cerr << "Failed to parse text format input" << std::endl;
    return 1;
  }

  std::string binary_output;
  if (!message.SerializeToString(&binary_output)) {
    std::cerr << "Failed to serialize message" << std::endl;
    return 1;
  }

  std::cout.write(binary_output.data(), binary_output.size());
  return 0;
}

template <typename T>
int Decode() {
  std::string binary_input = ReadAllFromFd(STDIN_FILENO);

  T message;
  if (!message.ParseFromString(binary_input)) {
    std::cerr << "Failed to parse binary input" << std::endl;
    return 1;
  }

  std::string text_output;
  if (!google::protobuf::TextFormat::PrintToString(message, &text_output)) {
    std::cerr << "Failed to print text format" << std::endl;
    return 1;
  }

  std::cout << text_output;
  return 0;
}

template <typename T>
int Roundtrip() {
  std::string text_input = ReadAllFromFd(STDIN_FILENO);

  T message1;
  if (!google::protobuf::TextFormat::ParseFromString(text_input, &message1)) {
    std::cerr << "Failed to parse text format input" << std::endl;
    return 1;
  }

  std::string binary;
  if (!message1.SerializeToString(&binary)) {
    std::cerr << "Failed to serialize message" << std::endl;
    return 1;
  }

  T message2;
  if (!message2.ParseFromString(binary)) {
    std::cerr << "Failed to parse binary" << std::endl;
    return 1;
  }

  // Compare using MessageDifferencer for canonical equality
  if (!google::protobuf::util::MessageDifferencer::Equals(message1, message2)) {
    std::cerr << "Roundtrip mismatch!" << std::endl;
    std::cerr << "Original:\n" << message1.DebugString() << std::endl;
    std::cerr << "After roundtrip:\n" << message2.DebugString() << std::endl;
    return 1;
  }

  std::cout.write(binary.data(), binary.size());
  std::cerr << "Roundtrip OK (" << binary.size() << " bytes)" << std::endl;
  return 0;
}

template <typename T>
int RunWithMessage(const std::string& mode) {
  if (mode == "encode") {
    return Encode<T>();
  } else if (mode == "decode") {
    return Decode<T>();
  } else if (mode == "roundtrip") {
    return Roundtrip<T>();
  } else {
    std::cerr << "Unknown mode: " << mode << std::endl;
    return 1;
  }
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
  std::string message = absl::GetFlag(FLAGS_message);

  if (message == "TestMessage") {
    return RunWithMessage<fuzztest::TestMessage>(mode);
  } else if (message == "NestedExample") {
    return RunWithMessage<fuzztest::NestedExample>(mode);
  } else {
    std::cerr << "Unknown message type: " << message << std::endl;
    std::cerr << "Available: TestMessage, NestedExample" << std::endl;
    return 1;
  }
}
