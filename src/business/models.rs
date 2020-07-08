use slice_as_array;
use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Debug)]
enum ResponseCode {
    NoError,
    FormatError,
    ServerFailure,
    NameError,
    NotImplemented,
    Refused,
}

#[derive(Debug)]
enum OpCode {
    Query,
    IQuery,
    Status,
    Notify,
    Update,
}

// https://tools.ietf.org/html/rfc1035 3.2.2
#[derive(Debug)]
enum Type {
    A(Ipv4Addr),    // Host address. 1
    NS(String),     // Authoritative name server for the domain. 2
    CNAME(String),  // Canonical name of an alias. 5
    SOA,            // Identifies the start of zone of authority. 6
    PTR,            // A pointer to another part of the domain name space. 12
    HINFO,          // host information, 13
    MX,             // Identifies a mail exchange for domain. 15
    AAAA(Ipv6Addr), // ipv6 28
    TXT,            // Text strings
}

// https://tools.ietf.org/html/rfc1035 3.2.3
#[derive(Debug)]
enum QType {
    A,     // Host address. 1
    NS,    // Authoritative name server for the domain. 2
    CNAME, // Canonical name of an alias. 5
    SOA,   // Identifies the start of zone of authority. 6
    PTR,   // A pointer to another part of the domain name space. 12
    MX,    // Identifies a mail exchange for domain. 15
    AAAA,  // ipv6 28
    TXT,   // Text strings 16
    AXFR,  // A request for a transfer of an entier zone. 252
    MAILB, // A request for mailbox-related records (MB, MG or MR). 253
    MAILA, // A request for mail agent RRs (obsolete - see MX). 254
    STAR,  // (*) A request for all records, 255
}

impl QType {
    fn decimal_to_qtype(value: u16) -> QType {
        match value {
            1 => QType::A,
            2 => QType::NS,
            5 => QType::CNAME,
            6 => QType::SOA,
            12 => QType::PTR,
            15 => QType::MX,
            16 => QType::TXT,
            28 => QType::AAAA,
            252 => QType::AXFR,
            253 => QType::MAILB,
            254 => QType::MAILA,
            255 => QType::STAR,
            _ => QType::STAR,
        }
    }
}

#[derive(Debug)]
enum Class {
    IN, // 1 the internet
    CH, // 3 the CHAOS class
}

impl Class {
    fn to_class(code: u16) -> Class {
        match code {
            1 => Class::IN,
            3 => Class::CH,
            _ => Class::IN,
        }
    }
}

#[derive(Debug)]
enum QClass {
    IN,   // Internet system
    CH,   // Chaos system
    STAR, // Any class
}

// Business models.
#[derive(Debug)]
struct DNSQueryHeaderSection {
    id: u16, // 2B, [0-15]

    // Flags.
    is_query: bool,                // 1b, 16
    op_code: OpCode,               // 4b, [17-20]
    is_authoritative_answer: bool, // 1b, 21
    is_truncated: bool,            // 1b, 22
    is_recursion_desired: bool,    // 1b, 23
    is_recursion_available: bool,  // 1b, 24
    response_code: ResponseCode,   // 4b, [28-31]

    questions_count: u16,     // 2B, [32-47]
    answers_count: u16,       // 2B, [48-63]
    ns_rr_count: u16,         // 2B, [64-79]
    additional_rr_count: u16, // 2B, [80-95]
}

#[derive(Debug)]
struct DNSQuestionQuery {
    qname: String,
    qtype: QType,
    qclass: QClass,
}

#[derive(Debug)]
struct ResourceRecord {
    name: String, // owner name to which this record pertains.
    r#type: Type, // 2 octets
    class: Class, // 2 octets
    ttl: u32,     // 4 octets, in seconds, 0 signifies indefinite caching.
    rd_length: u16,
}

#[derive(Debug)]
struct DNSQueryResponse {
    dns_query: DNSQuery,
    answers: Vec<ResourceRecord>,
    authority: Vec<ResourceRecord>,
    additional: Vec<ResourceRecord>,
}

#[derive(Debug)]
pub struct DNSQuery {
    header: DNSQueryHeaderSection,
    questions: Vec<DNSQuestionQuery>,
    answers: Vec<ResourceRecord>,
    authority: Vec<ResourceRecord>,
    additional: Vec<ResourceRecord>,

    pub buf: Vec<u8>,
}

impl DNSQuery {
    pub fn transform_to_wire_format(buf: &[u8]) -> DNSQuery {
        let header_section = DNSQuery::parse_header_section(buf);

        let (question_section, offset) =
            DNSQuery::parse_question_section(buf, header_section.questions_count);

        // Read answers.
        let (answer_section, offset) =
            DNSQuery::parse_resource_records(buf, offset, header_section.answers_count);

        let (authority_section, offset) =
            DNSQuery::parse_resource_records(buf, offset, header_section.ns_rr_count);

        let (additional_section, _) =
            DNSQuery::parse_resource_records(buf, offset, header_section.additional_rr_count);

        DNSQuery {
            header: header_section,
            questions: question_section,
            answers: answer_section,
            authority: authority_section,
            additional: additional_section,
            buf: buf.iter().cloned().collect(),
        }
    }

    // https://tools.ietf.org/html/rfc1035 Section 4.1.1
    fn parse_header_section(buf: &[u8]) -> DNSQueryHeaderSection {
        let id_array = slice_as_array!(&buf[..2], [u8; 2]).expect("bad slice length");
        let id = u16::from_be_bytes(*id_array);
        let is_query = !is_ith_bit_set(buf, 16);
        let op_code = get_op_code(buf);
        let is_authoritative_answer = is_ith_bit_set(buf, 21);
        let is_truncated = is_ith_bit_set(buf, 22);
        let is_recursion_desired = is_ith_bit_set(buf, 23);
        let is_recursion_available = is_ith_bit_set(buf, 24);
        let response_code = get_response_code(buf);
        let questions_count = get_questions_count(buf);
        let answers_count = get_answers_count(buf);
        let ns_rr_count = get_ns_rr_count(buf);
        let additional_rr_count = get_additional_rr_count(buf);
        DNSQueryHeaderSection {
            id,
            is_query,
            op_code,
            is_authoritative_answer,
            is_truncated,
            is_recursion_desired,
            is_recursion_available,
            response_code,
            questions_count,
            answers_count,
            ns_rr_count,
            additional_rr_count,
        }
    }

    fn parse_question_section(buf: &[u8], questions_count: u16) -> (Vec<DNSQuestionQuery>, usize) {
        let mut queries = Vec::new();
        let mut index = 96 / 8;
        for _ in 0..questions_count {
            let mut labels = Vec::new();
            loop {
                let octet_length = buf[index];
                if octet_length == 0 {
                    break;
                }
                let label_bytes = &buf[index + 1..index + 1 + octet_length as usize];
                let label = std::str::from_utf8(label_bytes).unwrap();
                labels.push(label);
                index += octet_length as usize + 1;
            }
            let qtype_u16 = convert_slice_to_u16(&buf[index + 1..index + 3]);
            let query = DNSQuestionQuery {
                qname: String::from(labels.join(".")),
                qtype: QType::decimal_to_qtype(qtype_u16),
                qclass: QClass::IN,
            };
            queries.push(query);

            // move index by four octets: two octets for type and two for class.
            index += 5;
        }

        (queries, index)
    }

    fn parse_resource_records(
        buf: &[u8],
        mut index: usize,
        rr_count: u16,
    ) -> (Vec<ResourceRecord>, usize) {
        if rr_count == 0 {
            return (vec![], index);
        }

        let mut answers = Vec::new();

        // parse name.
        println!(
            "index={} buf.len={} rr_count={}",
            index,
            buf.len(),
            rr_count
        );
        for _ in 0..rr_count {
            // parse name

            let (name, updated_index) = read_labels(&buf, index);
            index = updated_index;

            // parse type.
            let type_code: u16 = convert_slice_to_u16(&buf[index..index + 2]);
            index += 2;

            // parse class.
            let class_code: u16 = convert_slice_to_u16(&buf[index..index + 2]);
            index += 2;

            // parse ttl.
            let ttl: u32 = convert_slice_to_u32(&buf[index..index + 4]);
            index += 4;

            // parse rdlength
            let rd_length: u16 = convert_slice_to_u16(&buf[index..index + 2]);
            index += 2;

            // parse rr data
            let rd_data: &[u8] = &buf[index..index + rd_length as usize];

            let type_with_data: Type = match type_code {
                1 => {
                    let ipv4_addr = Ipv4Addr::new(rd_data[0], rd_data[1], rd_data[2], rd_data[3]);
                    Type::A(ipv4_addr)
                }
                2 => {
                    let (ns_dname, _) = read_labels(buf, index);
                    Type::NS(ns_dname)
                }
                5 => {
                    let (cname, _) = read_labels(buf, index);
                    Type::CNAME(cname)
                }
                6 => Type::SOA,
                12 => Type::PTR,
                15 => Type::MX,
                16 => Type::TXT,
                28 => {
                    let ipv6_addr = Ipv6Addr::new(
                        convert_slice_to_u16(&rd_data[0..2]),
                        convert_slice_to_u16(&rd_data[2..4]),
                        convert_slice_to_u16(&rd_data[4..6]),
                        convert_slice_to_u16(&rd_data[6..8]),
                        convert_slice_to_u16(&rd_data[8..10]),
                        convert_slice_to_u16(&rd_data[10..12]),
                        convert_slice_to_u16(&rd_data[12..14]),
                        convert_slice_to_u16(&rd_data[14..16]),
                    );
                    Type::AAAA(ipv6_addr)
                }
                _ => Type::TXT,
            };

            let answer = ResourceRecord {
                name,
                r#type: type_with_data,
                class: Class::to_class(class_code),
                ttl,
                rd_length,
            };
            answers.push(answer);
            index += rd_length as usize;
        }

        (answers, index)
    }
}

fn read_labels(buf: &[u8], offset: usize) -> (String, usize) {
    let mut index = offset;
    let mut labels = Vec::new();
    loop {
        let octet_length = buf[index];
        if octet_length & 0xC0 == 0xC0 {
            // we have encountered a pointer.
            let pointer_to_label = convert_slice_to_u16(&buf[index..index + 2]) & 0x3FFF;
            let (labels_from_pointer, _) = read_labels(&buf, pointer_to_label as usize);
            labels.push(labels_from_pointer);
            index += 1;
            break;
        }
        if octet_length == 0 {
            break;
        }
        let label_bytes = &buf[index + 1..index + 1 + octet_length as usize];
        let label = std::str::from_utf8(label_bytes).unwrap();
        labels.push(String::from(label));
        index += octet_length as usize + 1;
    }
    (labels.join("."), index + 1)
}

fn get_response_code(buf: &[u8]) -> ResponseCode {
    let index_in_buf: usize = 28 / 8;
    let response_code = buf[index_in_buf] & 15;
    match response_code {
        0 => ResponseCode::NoError,
        1 => ResponseCode::FormatError,
        2 => ResponseCode::ServerFailure,
        3 => ResponseCode::NameError,
        4 => ResponseCode::NotImplemented,
        5 => ResponseCode::Refused,
        _ => ResponseCode::FormatError, // TODO: fix this.
    }
}

fn get_questions_count(buf: &[u8]) -> u16 {
    let index: usize = 32 / 8;
    let qdcount_bytes = slice_as_array!(&buf[index..index + 2], [u8; 2]).expect("bad slice length");
    u16::from_be_bytes(*qdcount_bytes)
}

fn get_answers_count(buf: &[u8]) -> u16 {
    let index: usize = 48 / 8;
    let ancount_bytes = slice_as_array!(&buf[index..index + 2], [u8; 2]).expect("bad slice length");
    u16::from_be_bytes(*ancount_bytes)
}

fn get_ns_rr_count(buf: &[u8]) -> u16 {
    let index: usize = 64 / 8;
    let ns_records_count_bytes =
        slice_as_array!(&buf[index..index + 2], [u8; 2]).expect("bad slice length");
    u16::from_be_bytes(*ns_records_count_bytes)
}

fn get_additional_rr_count(buf: &[u8]) -> u16 {
    let index: usize = 80 / 8;
    let additional_rr_count_bytes =
        slice_as_array!(&buf[index..index + 2], [u8; 2]).expect("bad slice length");
    u16::from_be_bytes(*additional_rr_count_bytes)
}

fn get_op_code(buf: &[u8]) -> OpCode {
    let index_in_buf: usize = 17 / 8;
    let op_code = buf[index_in_buf] & 120;
    match op_code {
        0 => OpCode::Query,
        1 => OpCode::IQuery,
        2 => OpCode::Status,
        4 => OpCode::Notify,
        5 => OpCode::Update,
        _ => OpCode::Query, // TODO: handle correctly.
    }
}

// 2 octets to u16.
fn convert_slice_to_u16(slice: &[u8]) -> u16 {
    let slice_as_array = slice_as_array!(&slice[..2], [u8; 2]).expect("bad slice length");
    u16::from_be_bytes(*slice_as_array)
}

fn convert_slice_to_u32(slice: &[u8]) -> u32 {
    let slice_as_array = slice_as_array!(&slice[..4], [u8; 4]).expect("bad slice length");
    u32::from_be_bytes(*slice_as_array)
}

// Assumes i < len(buf) * 8.
fn is_ith_bit_set(buf: &[u8], i: usize) -> bool {
    let index_in_buf: usize = i / 8;
    let index_in_byte: usize = i % 8;
    (buf[index_in_buf] & (1 << (7 - index_in_byte))) != 0
}
