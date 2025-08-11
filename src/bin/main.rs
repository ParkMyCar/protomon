#[derive(Clone, prost::Message)]
pub struct MyField {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// Unique ID number for this person.
    #[prost(int32, tag = "2")]
    pub id: i32,
    #[prost(string, tag = "3")]
    pub email: ::prost::alloc::string::String,
    #[prost(message, repeated, tag = "4")]
    pub phones: ::prost::alloc::vec::Vec<PhoneNumber>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PhoneNumber {
    #[prost(string, tag = "1")]
    pub number: ::prost::alloc::string::String,
}

fn main() {
    println!("hello world");
}
