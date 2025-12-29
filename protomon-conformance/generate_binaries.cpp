// Conformance test binary generator.
//
// This tool reads all test case definitions and generates the corresponding
// binary protobuf files using the C++ protobuf library as the reference
// implementation.
//
// Usage:
//   bazel run //conformance:generate_binaries -- --output_dir=/path/to/testdata

#include <fstream>
#include <iostream>
#include <sstream>
#include <string>
#include <vector>

#include <sys/stat.h>
#include <sys/types.h>

#include "absl/flags/flag.h"
#include "absl/flags/parse.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/str_split.h"
#include "google/protobuf/text_format.h"

// Include all conformance proto headers
#include "protos/scalars.pb.h"
#include "protos/repeated.pb.h"
#include "protos/nested.pb.h"
#include "protos/edge_cases.pb.h"

ABSL_FLAG(std::string, output_dir, "", "Output directory for binary files");
ABSL_FLAG(std::string, input_dir, "", "Input directory containing testdata");

// Simple path join
std::string JoinPath(const std::string& a, const std::string& b) {
  if (a.empty()) return b;
  if (b.empty()) return a;
  if (a.back() == '/') return a + b;
  return a + "/" + b;
}

// Get directory part of path
std::string DirName(const std::string& path) {
  auto pos = path.rfind('/');
  if (pos == std::string::npos) return ".";
  return path.substr(0, pos);
}

// Create directory (including parents)
bool MkdirP(const std::string& path) {
  std::string current;
  for (char c : path) {
    current += c;
    if (c == '/') {
      mkdir(current.c_str(), 0755);
    }
  }
  mkdir(path.c_str(), 0755);
  return true;
}

// Check if file exists
bool FileExists(const std::string& path) {
  struct stat st;
  return stat(path.c_str(), &st) == 0;
}

// Helper to read a file into a string
std::string ReadFile(const std::string& path) {
  std::ifstream file(path);
  if (!file) {
    std::cerr << "Failed to open: " << path << std::endl;
    return "";
  }
  std::stringstream buffer;
  buffer << file.rdbuf();
  return buffer.str();
}

// Helper to write binary data to a file
bool WriteFile(const std::string& path, const std::string& data) {
  // Create parent directory
  MkdirP(DirName(path));

  std::ofstream file(path, std::ios::binary);
  if (!file) {
    std::cerr << "Failed to create: " << path << std::endl;
    return false;
  }
  file.write(data.data(), data.size());
  return true;
}

// Template function to process a single test case
template <typename MessageType>
bool ProcessTestCase(const std::string& textproto_path, const std::string& bin_path) {
  std::string text_content = ReadFile(textproto_path);
  if (text_content.empty() && !FileExists(textproto_path)) {
    return false;
  }

  MessageType message;
  if (!google::protobuf::TextFormat::ParseFromString(text_content, &message)) {
    std::cerr << "Failed to parse: " << textproto_path << std::endl;
    return false;
  }

  std::string binary;
  if (!message.SerializeToString(&binary)) {
    std::cerr << "Failed to serialize: " << textproto_path << std::endl;
    return false;
  }

  if (!WriteFile(bin_path, binary)) {
    return false;
  }

  std::cout << "Generated: " << bin_path << " (" << binary.size() << " bytes)" << std::endl;
  return true;
}

// Process all test cases in a category
bool ProcessCategory(const std::string& input_dir, const std::string& output_dir,
                     const std::string& category) {
  std::string tests_file = JoinPath(JoinPath(input_dir, category), "tests.txt");

  std::ifstream file(tests_file);
  if (!file) {
    std::cerr << "No tests.txt found: " << tests_file << std::endl;
    return true;  // Not an error, just skip
  }

  std::string line;
  int success_count = 0;
  int fail_count = 0;

  while (std::getline(file, line)) {
    // Skip empty lines and comments
    if (line.empty() || line[0] == '#') continue;

    // Parse "test_name MessageType"
    std::vector<std::string> parts = absl::StrSplit(line, ' ');
    if (parts.size() != 2) {
      std::cerr << "Invalid line in tests.txt: " << line << std::endl;
      continue;
    }

    std::string test_name = parts[0];
    std::string message_type = parts[1];

    std::string category_dir = JoinPath(input_dir, category);
    std::string textproto_path = JoinPath(category_dir, test_name + ".textproto");
    std::string bin_path = JoinPath(JoinPath(output_dir, category), test_name + ".bin");

    // Dispatch based on message type
    bool ok = false;

    // Scalars
    if (message_type == "Scalars") ok = ProcessTestCase<conformance::Scalars>(textproto_path, bin_path);
    else if (message_type == "Int32Value") ok = ProcessTestCase<conformance::Int32Value>(textproto_path, bin_path);
    else if (message_type == "Int64Value") ok = ProcessTestCase<conformance::Int64Value>(textproto_path, bin_path);
    else if (message_type == "Uint32Value") ok = ProcessTestCase<conformance::Uint32Value>(textproto_path, bin_path);
    else if (message_type == "Uint64Value") ok = ProcessTestCase<conformance::Uint64Value>(textproto_path, bin_path);
    else if (message_type == "Sint32Value") ok = ProcessTestCase<conformance::Sint32Value>(textproto_path, bin_path);
    else if (message_type == "Sint64Value") ok = ProcessTestCase<conformance::Sint64Value>(textproto_path, bin_path);
    else if (message_type == "BoolValue") ok = ProcessTestCase<conformance::BoolValue>(textproto_path, bin_path);
    else if (message_type == "Fixed32Value") ok = ProcessTestCase<conformance::Fixed32Value>(textproto_path, bin_path);
    else if (message_type == "Sfixed32Value") ok = ProcessTestCase<conformance::Sfixed32Value>(textproto_path, bin_path);
    else if (message_type == "Fixed64Value") ok = ProcessTestCase<conformance::Fixed64Value>(textproto_path, bin_path);
    else if (message_type == "Sfixed64Value") ok = ProcessTestCase<conformance::Sfixed64Value>(textproto_path, bin_path);
    else if (message_type == "FloatValue") ok = ProcessTestCase<conformance::FloatValue>(textproto_path, bin_path);
    else if (message_type == "DoubleValue") ok = ProcessTestCase<conformance::DoubleValue>(textproto_path, bin_path);
    else if (message_type == "StringValue") ok = ProcessTestCase<conformance::StringValue>(textproto_path, bin_path);
    else if (message_type == "BytesValue") ok = ProcessTestCase<conformance::BytesValue>(textproto_path, bin_path);

    // Repeated
    else if (message_type == "RepeatedScalars") ok = ProcessTestCase<conformance::RepeatedScalars>(textproto_path, bin_path);
    else if (message_type == "RepeatedInt32") ok = ProcessTestCase<conformance::RepeatedInt32>(textproto_path, bin_path);
    else if (message_type == "RepeatedInt64") ok = ProcessTestCase<conformance::RepeatedInt64>(textproto_path, bin_path);
    else if (message_type == "RepeatedUint32") ok = ProcessTestCase<conformance::RepeatedUint32>(textproto_path, bin_path);
    else if (message_type == "RepeatedUint64") ok = ProcessTestCase<conformance::RepeatedUint64>(textproto_path, bin_path);
    else if (message_type == "RepeatedSint32") ok = ProcessTestCase<conformance::RepeatedSint32>(textproto_path, bin_path);
    else if (message_type == "RepeatedSint64") ok = ProcessTestCase<conformance::RepeatedSint64>(textproto_path, bin_path);
    else if (message_type == "RepeatedBool") ok = ProcessTestCase<conformance::RepeatedBool>(textproto_path, bin_path);
    else if (message_type == "RepeatedFixed32") ok = ProcessTestCase<conformance::RepeatedFixed32>(textproto_path, bin_path);
    else if (message_type == "RepeatedSfixed32") ok = ProcessTestCase<conformance::RepeatedSfixed32>(textproto_path, bin_path);
    else if (message_type == "RepeatedFixed64") ok = ProcessTestCase<conformance::RepeatedFixed64>(textproto_path, bin_path);
    else if (message_type == "RepeatedSfixed64") ok = ProcessTestCase<conformance::RepeatedSfixed64>(textproto_path, bin_path);
    else if (message_type == "RepeatedFloat") ok = ProcessTestCase<conformance::RepeatedFloat>(textproto_path, bin_path);
    else if (message_type == "RepeatedDouble") ok = ProcessTestCase<conformance::RepeatedDouble>(textproto_path, bin_path);
    else if (message_type == "RepeatedString") ok = ProcessTestCase<conformance::RepeatedString>(textproto_path, bin_path);
    else if (message_type == "RepeatedBytes") ok = ProcessTestCase<conformance::RepeatedBytes>(textproto_path, bin_path);

    // Nested
    else if (message_type == "Outer") ok = ProcessTestCase<conformance::Outer>(textproto_path, bin_path);
    else if (message_type == "Level0") ok = ProcessTestCase<conformance::Level0>(textproto_path, bin_path);
    else if (message_type == "Node") ok = ProcessTestCase<conformance::Node>(textproto_path, bin_path);
    else if (message_type == "OptionalNested") ok = ProcessTestCase<conformance::OptionalNested>(textproto_path, bin_path);

    // Edge cases
    else if (message_type == "FieldNumbers") ok = ProcessTestCase<conformance::FieldNumbers>(textproto_path, bin_path);
    else if (message_type == "WireTypes") ok = ProcessTestCase<conformance::WireTypes>(textproto_path, bin_path);
    else if (message_type == "Empty") ok = ProcessTestCase<conformance::Empty>(textproto_path, bin_path);
    else if (message_type == "AllDefaults") ok = ProcessTestCase<conformance::AllDefaults>(textproto_path, bin_path);
    else if (message_type == "OptionalFields") ok = ProcessTestCase<conformance::OptionalFields>(textproto_path, bin_path);

    else {
      std::cerr << "Unknown message type: " << message_type << std::endl;
      fail_count++;
      continue;
    }

    if (ok) {
      success_count++;
    } else {
      fail_count++;
    }
  }

  std::cout << "Category " << category << ": " << success_count << " succeeded, "
            << fail_count << " failed" << std::endl;
  return fail_count == 0;
}

int main(int argc, char* argv[]) {
  absl::ParseCommandLine(argc, argv);

  std::string output_dir = absl::GetFlag(FLAGS_output_dir);
  std::string input_dir = absl::GetFlag(FLAGS_input_dir);

  if (output_dir.empty()) {
    std::cerr << "Error: --output_dir is required" << std::endl;
    return 1;
  }

  if (input_dir.empty()) {
    // Default to the same as output_dir
    input_dir = output_dir;
  }

  std::cout << "Input directory: " << input_dir << std::endl;
  std::cout << "Output directory: " << output_dir << std::endl;

  bool all_ok = true;
  all_ok &= ProcessCategory(input_dir, output_dir, "scalars");
  all_ok &= ProcessCategory(input_dir, output_dir, "repeated");
  all_ok &= ProcessCategory(input_dir, output_dir, "nested");
  all_ok &= ProcessCategory(input_dir, output_dir, "edge_cases");

  if (all_ok) {
    std::cout << "\nAll test cases generated successfully!" << std::endl;
    return 0;
  } else {
    std::cerr << "\nSome test cases failed to generate." << std::endl;
    return 1;
  }
}
