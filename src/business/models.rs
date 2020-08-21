use itertools::interleave;
use serde::{Deserialize, Serialize};
use slice_as_array::{slice_as_array, slice_as_array_transmute};
use std::cmp::Eq;
use std::fmt;
use std::hash::Hash;
use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, PartialEq)]
pub enum ResponseCode {
    NoError,     // No error condition, 0
    FormatError, // Format error - 1
    // ServerFailure, // Name Error - 3
    NameError,      // 3, No such name.
    NotImplemented, // 4
    Refused,        // 5
}

impl ResponseCode {
    fn to_u8(&self) -> u8 {
        match *self {
            ResponseCode::NoError => 0,
            ResponseCode::FormatError => 1,
            ResponseCode::NameError => 3,
            ResponseCode::NotImplemented => 4,
            ResponseCode::Refused => 5,
        }
    }
}

#[derive(Debug, Clone)]
pub enum OpCode {
    Query,
    IQuery,
    Status,
    Notify,
    Update,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MXData {
    preference: u16,
    exchange: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TXTData {
    length: u8,
    data: String,
}

impl TXTData {
    // length: u8
    // data: string of length u8
    fn serialize(&self) -> Vec<u8> {
        let mut result = vec![self.length];

        result.append(&mut self.data.as_bytes().to_vec());
        result
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SOAData {
    mname: String,
    rname: String,
    serial: u32,
    refresh_in_secs: u32,
    retry_in_secs: u32,
    expire_in_secs: u32,
    minimum: u32,
}

impl SOAData {
    fn serialize(&self) -> Vec<u8> {
        let mut result: Vec<u8> = vec![];

        let mut mname = write_labels(self.mname.as_str());
        result.append(&mut mname);

        let mut rname = write_labels(self.rname.as_str());
        result.append(&mut rname);

        let serial = transform_u32_to_array_of_u8(self.serial);
        serial
            .iter()
            .for_each(|num: &u8| result.append(&mut vec![num.clone()]));

        let refresh_in_secs = transform_u32_to_array_of_u8(self.refresh_in_secs);
        refresh_in_secs
            .iter()
            .for_each(|num: &u8| result.append(&mut vec![num.clone()]));

        let retry_in_secs = transform_u32_to_array_of_u8(self.retry_in_secs);
        retry_in_secs
            .iter()
            .for_each(|num: &u8| result.append(&mut vec![num.clone()]));

        let expire_in_secs = transform_u32_to_array_of_u8(self.expire_in_secs);
        expire_in_secs
            .iter()
            .for_each(|num: &u8| result.append(&mut vec![num.clone()]));

        let minimum = transform_u32_to_array_of_u8(self.minimum);
        minimum
            .iter()
            .for_each(|num: &u8| result.append(&mut vec![num.clone()]));

        result
    }

    fn deserialize(buf: &[u8], offset: usize, _length: u16) -> Self {
        let (mname, start_of_rname) = read_labels(buf, offset);
        let (rname, start_of_serial) = read_labels(buf, start_of_rname);
        let j = start_of_serial;
        let serial = convert_slice_to_u32(&buf[j..j + 4]);
        let refresh_in_secs = convert_slice_to_u32(&buf[j + 4..j + 8]);
        let retry_in_secs = convert_slice_to_u32(&buf[j + 8..j + 12]);
        let expire_in_secs = convert_slice_to_u32(&buf[j + 12..j + 16]);
        let minimum = convert_slice_to_u32(&buf[j + 16..j + 20]);
        Self {
            mname,
            rname,
            serial,
            refresh_in_secs,
            retry_in_secs,
            expire_in_secs,
            minimum,
        }
    }
}

// https://tools.ietf.org/html/rfc1035 3.2.2
// Type is used in ResourceRecords.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Type {
    A(Ipv4Addr),    // Host address. 1
    NS(String),     // Authoritative name server for the domain. 2
    CNAME(String),  // Canonical name of an alias. 5
    SOA(SOAData),   // Identifies the start of zone of authority. 6
    PTR(String),    // A pointer to another part of the domain name space. 12
    HINFO,          // host information, 13
    MX(MXData),     // Identifies a mail exchange for domain. 15
    AAAA(Ipv6Addr), // ipv6 28
    TXT(TXTData),   // Text strings
}

impl Type {
    pub fn to_qtype(&self) -> QType {
        match *self {
            Type::A(_) => QType::A,
            Type::NS(_) => QType::NS,
            Type::CNAME(_) => QType::CNAME,
            Type::SOA(_) => QType::SOA,
            Type::PTR(_) => QType::PTR,
            Type::HINFO => QType::HINFO,
            Type::MX(_) => QType::MX,
            Type::AAAA(_) => QType::AAAA,
            Type::TXT(_) => QType::TXT,
        }
    }

    fn to_u16(&self) -> u16 {
        match *self {
            Type::A(_) => 1,
            Type::NS(_) => 2,
            Type::CNAME(_) => 5,
            Type::SOA(_) => 6,
            Type::PTR(_) => 12,
            Type::HINFO => 13,
            Type::MX(_) => 15,
            Type::AAAA(_) => 28,
            Type::TXT(_) => 16,
        }
    }
}

// https://tools.ietf.org/html/rfc1035 3.2.3
#[derive(Debug, PartialEq, Clone, Copy, Hash, Eq, Serialize, Deserialize)]
pub enum QType {
    A,     // Host address. 1
    NS,    // Authoritative name server for the domain. 2
    CNAME, // Canonical name of an alias. 5
    SOA,   // Identifies the start of zone of authority. 6
    PTR,   // A pointer to another part of the domain name space. 12
    HINFO, // Host information. 13
    MX,    // Identifies a mail exchange for domain. 15
    AAAA,  // ipv6 28
    TXT,   // Text strings 16
    AXFR,  // A request for a transfer of an entier zone. 252
    MAILB, // A request for mailbox-related records (MB, MG or MR). 253
    MAILA, // A request for mail agent RRs (obsolete - see MX). 254
    STAR,  // (*) A request for all records, 255 - TODO: OBSOLETE
}

struct InvalidTypeError {
    code: u16,
}

impl fmt::Display for InvalidTypeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid type error: {}", self.code)
    }
}

impl fmt::Debug for InvalidTypeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid type error")
    }
}

impl QType {
    fn decimal_to_qtype(value: u16) -> Result<QType, InvalidTypeError> {
        match value {
            1 => Ok(QType::A),
            2 => Ok(QType::NS),
            5 => Ok(QType::CNAME),
            6 => Ok(QType::SOA),
            12 => Ok(QType::PTR),
            15 => Ok(QType::MX),
            16 => Ok(QType::TXT),
            28 => Ok(QType::AAAA),
            252 => Ok(QType::AXFR),
            253 => Ok(QType::MAILB),
            254 => Ok(QType::MAILA),
            255 => Ok(QType::STAR),
            _ => Err(InvalidTypeError { code: value }),
        }
    }

    fn to_u16(&self) -> u16 {
        match *self {
            QType::A => 1,
            QType::NS => 2,
            QType::CNAME => 5,
            QType::SOA => 6,
            QType::PTR => 12,
            QType::HINFO => 13,
            QType::MX => 15,
            QType::TXT => 16,
            QType::AAAA => 28,
            QType::AXFR => 252,
            QType::MAILB => 253,
            QType::MAILA => 254,
            QType::STAR => 255,
        }
    }
}

impl fmt::Display for QType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:?}",
            match *self {
                QType::A => "A",
                QType::NS => "NS",
                QType::CNAME => "CNAME",
                QType::SOA => "SOA",
                QType::PTR => "PTR",
                QType::HINFO => "HINFO",
                QType::MX => "MX",
                QType::TXT => "TXT",
                QType::AAAA => "AAAA",
                QType::AXFR => "AXFR",
                QType::MAILB => "MAILB",
                QType::MAILA => "MAILA",
                QType::STAR => "STAR",
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Class {
    IN, // 1 the internet
    CH, // 3 the CHAOS class
}

impl Class {
    fn to_u16(&self) -> u16 {
        match *self {
            Class::IN => 1,
            Class::CH => 3,
        }
    }
}

struct InvalidClassError {
    code: u16,
}

impl fmt::Display for InvalidClassError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid class error: {}", self.code)
    }
}
impl fmt::Debug for InvalidClassError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid class error: {}", self.code)
    }
}

impl Class {
    fn to_class(code: u16) -> Result<Class, InvalidClassError> {
        match code {
            1 => Ok(Class::IN),
            3 => Ok(Class::CH),
            _ => Err(InvalidClassError { code }),
        }
    }
}

#[derive(Debug, Clone)]
pub enum QClass {
    IN,   // Internet system
    CH,   // Chaos system
    STAR, // Any class
}

impl QClass {
    fn to_u16(&self) -> u16 {
        match *self {
            QClass::IN => 1,
            QClass::CH => 2, // ???
            QClass::STAR => 255,
        }
    }
}

// Business models.

// Represents list of RR of same type and class.
pub type RRSet = Vec<ResourceRecord>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRecord {
    pub name: String,   // owner name to which this record pertains.
    pub r#type: Type,   // 2 octets, data will be inside Type enum.
    pub class: Class,   // 2 octets
    pub ttl: u32,       // 4 octets, in seconds, 0 signifies indefinite caching.
    pub rd_length: u16, // TODO: remove pub.
}

impl ResourceRecord {
    fn serialize(&self) -> Vec<u8> {
        let serialized_name = write_labels(&self.name);

        let mut serialized_type = Vec::with_capacity(2);
        serialized_type.push(((self.r#type.to_u16() >> 8) & 0xff) as u8);
        serialized_type.push((self.r#type.to_u16() & 0xff) as u8);

        let serialized_rdata = match &self.r#type {
            Type::A(ipv4) => ipv4.octets().to_vec(),
            Type::AAAA(ipv6) => ipv6.octets().to_vec(),
            Type::NS(ns) => write_labels(&ns),
            Type::TXT(txt_data) => txt_data.serialize(),
            Type::CNAME(cname) => write_labels(cname),
            Type::SOA(soa) => soa.serialize(),
            _ => panic!(
                "ResourceRecord:serialize type not supported: {:#?}",
                &self.r#type
            ),
        };

        let mut serialized_class = Vec::with_capacity(2);
        serialized_class.push(((self.class.to_u16() >> 8) & 0xff) as u8);
        serialized_class.push((self.class.to_u16() & 0xff) as u8);

        let mut serialized_ttl = Vec::with_capacity(4);
        serialized_ttl.push(((self.ttl >> 24) & 0xff) as u8);
        serialized_ttl.push(((self.ttl >> 16) & 0xff) as u8);
        serialized_ttl.push(((self.ttl >> 8) & 0xff) as u8);
        serialized_ttl.push((self.ttl & 0xff) as u8);

        let mut serialized_rd_length = Vec::with_capacity(2);
        // TODO: implement compression of labels.
        let uncompressed_rd_length = serialized_rdata.len();
        serialized_rd_length.push(((uncompressed_rd_length >> 8) & 0xff) as u8);
        serialized_rd_length.push((uncompressed_rd_length & 0xff) as u8);

        itertools::concat(vec![
            serialized_name,
            serialized_type,
            serialized_class,
            serialized_ttl,
            serialized_rd_length,
            serialized_rdata,
        ])
    }
}

impl PartialEq for ResourceRecord {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.r#type == other.r#type && self.class == other.class
    }
}

#[derive(Debug, Clone)]
pub struct DNSQueryResponse {
    pub query: DNSQuery,
    pub answers: Vec<ResourceRecord>,
    pub authority: Vec<ResourceRecord>,
    pub additional: Vec<ResourceRecord>,
}

impl DNSQueryResponse {
    pub fn serialize(&self) -> Vec<u8> {
        let query = self.query.serialize();
        let answers = self.serialize_resource_records(&self.answers);
        let authority = self.serialize_resource_records(&self.authority);
        let additional = self.serialize_resource_records(&self.additional);
        itertools::concat(vec![query, answers, authority, additional])
    }

    fn serialize_resource_records(&self, resource_records: &Vec<ResourceRecord>) -> Vec<u8> {
        let serialized_rrs = resource_records
            .iter()
            .map(|rr| rr.serialize())
            .collect::<Vec<Vec<u8>>>();
        itertools::concat(serialized_rrs)
    }

    pub fn deserialize(data: &[u8]) -> DNSQueryResponse {
        let (query, answer_section_offset) = DNSQuery::deserialize(data);

        // Read answers.
        let (answer_section, authority_section_offset) = DNSQuery::deserialize_resource_records(
            data,
            answer_section_offset,
            query.header.answers_count,
        );

        // Read authority.
        let (authority_section, additional_section_offset) = DNSQuery::deserialize_resource_records(
            data,
            authority_section_offset,
            query.header.ns_rr_count,
        );

        // Read additional.
        let (additional_section, _) = DNSQuery::deserialize_resource_records(
            data,
            additional_section_offset,
            query.header.additional_rr_count,
        );

        DNSQueryResponse {
            query,
            answers: answer_section,
            authority: authority_section,
            additional: additional_section,
        }
    }

    pub fn contains_cnames(&self) -> Option<Vec<&ResourceRecord>> {
        let cname_rrs = self
            .answers
            .iter()
            .filter(|rr| {
                if rr.r#type.to_qtype() == QType::CNAME {
                    return true;
                }
                false
            })
            .collect::<Vec<&ResourceRecord>>();
        if cname_rrs.len() > 0 {
            return Some(cname_rrs);
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct DNSQueryHeaderSection {
    pub id: u16, // 2B, [0-15]

    // Flags.
    pub is_query: bool,                // 1b, 16
    pub op_code: OpCode,               // 4b, [17-20]
    pub is_authoritative_answer: bool, // 1b, 21
    pub is_truncated: bool,            // 1b, 22
    pub is_recursion_desired: bool,    // 1b, 23
    pub is_recursion_available: bool,  // 1b, 24
    pub response_code: ResponseCode,   // 4b, [28-31]

    pub questions_count: u16,     // 2B, [32-47]
    pub answers_count: u16,       // 2B, [48-63]
    pub ns_rr_count: u16,         // 2B, [64-79]
    pub additional_rr_count: u16, // 2B, [80-95]
}

impl DNSQueryHeaderSection {
    pub fn serialize(&self) -> Vec<u8> {
        let mut result = vec![];

        // id
        result.push(((self.id >> 8) & 0xff) as u8);
        result.push((self.id & 0xff) as u8);

        // flags
        let mut flags_first_byte: u8 = 0;

        // is_query = 0,1
        flags_first_byte = if !self.is_query {
            flags_first_byte ^ 0b10000000
        } else {
            flags_first_byte
        };

        // is_authoritative_answer = 5, 6
        flags_first_byte = if self.is_authoritative_answer {
            flags_first_byte ^ 0b00000100
        } else {
            flags_first_byte
        };
        // is_truncated = false/0 true/1 0 for now, 6,7

        // is_recursion_desired - 7,8
        flags_first_byte = if self.is_recursion_desired {
            flags_first_byte ^ 0b00000001
        } else {
            flags_first_byte
        };
        result.push(flags_first_byte);

        let mut flags_second_byte: u8 = 0;
        // is_recursion_available 0,1
        flags_second_byte = if self.is_recursion_available {
            flags_second_byte ^ 0b10000000
        } else {
            flags_second_byte
        };

        // response_code 4,8 [4,5,6,7]
        flags_second_byte = flags_second_byte ^ self.response_code.to_u8();
        result.push(flags_second_byte);

        // questions_count
        result.push(((self.questions_count >> 8) & 0xff) as u8);
        result.push((self.questions_count & 0xff) as u8);

        // answers_count
        result.push(((self.answers_count >> 8) & 0xff) as u8);
        result.push((self.answers_count & 0xff) as u8);

        // ns_rr_count
        result.push(((self.ns_rr_count >> 8) & 0xff) as u8);
        result.push((self.ns_rr_count & 0xff) as u8);

        // additional_rr_count
        result.push(((self.additional_rr_count >> 8) & 0xff) as u8);
        result.push((self.additional_rr_count & 0xff) as u8);

        result
    }
}

#[derive(Debug, Clone)]
pub struct DNSQuestionQuery {
    pub qname: String, // domain.
    pub qtype: QType,
    pub qclass: QClass,
}

impl DNSQuestionQuery {
    fn serialize(&self) -> Vec<u8> {
        let mut result = vec![];

        // qname
        let mut labels = self.serialize_domain(&self.qname);
        result.append(&mut labels);

        // qtype
        let qtype = self.qtype.to_u16();
        result.push(((qtype >> 8) & 0xff) as u8);
        result.push((qtype & 0xff) as u8);

        // qclass
        let qclass = self.qclass.to_u16();
        result.push(((qclass >> 8) & 0xff) as u8);
        result.push((qclass & 0xff) as u8);

        result
    }

    // domain = www.google.com
    // domain is split into "labels".
    // "labels"s' length is "label_lengths".
    // "label_lengths" and "labels" constitute of "label_part"s.
    // 3 | w | w | w | 6 | g | o | o | g | l | e | 3 | c | o | m | 0
    fn serialize_domain(&self, domain: &String) -> Vec<u8> {
        if domain == "." {
            return vec![0];
        }
        let labels: Vec<String> = domain
            .split('.')
            .filter(|x| x.len() > 0)
            .map(|label| label.to_string())
            .collect();
        let label_lengths: Vec<String> = domain
            .split('.')
            .filter(|x| x.len() > 0)
            .map(|label| label.len().to_string())
            .collect();
        let mut result = interleave(label_lengths, labels)
            .enumerate()
            .flat_map(|(i, val)| {
                if i % 2 == 0 {
                    return vec![val.parse::<u8>().unwrap()];
                }
                val.as_bytes().to_owned()
            })
            .collect::<Vec<u8>>();
        result.append(&mut vec![0]);
        result
    }
}

#[derive(Debug, Clone)]
pub struct DNSQuery {
    pub header: DNSQueryHeaderSection,
    // Number of questions will always be one.
    pub questions: Vec<DNSQuestionQuery>,
    pub additionals: Vec<ResourceRecord>,
}

impl DNSQuery {
    pub fn to_dig(&self) -> String {
        let question = &self.questions[0];
        format!("dig {} {}", question.qname, question.qtype)
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut result = vec![];

        let mut serialized_header = self.header.serialize();
        result.append(&mut serialized_header);
        let mut serialized_questions = self
            .questions
            .iter()
            .flat_map(|question| question.serialize())
            .collect::<Vec<u8>>();
        result.append(&mut serialized_questions);

        result
    }

    pub fn deserialize(buf: &[u8]) -> (DNSQuery, usize) {
        let header_section = DNSQuery::deserialize_header_section(buf);

        let (question_section, offset) =
            DNSQuery::deserialize_question_section(buf, header_section.questions_count);
        // let (additional_section, _) =
        //     DNSQuery::parse_resource_records(buf, offset, header_section.additional_rr_count);

        (
            Self {
                header: header_section,
                questions: question_section,
                additionals: vec![],
            },
            offset,
        )
    }

    // https://tools.ietf.org/html/rfc1035 Section 4.1.1
    fn deserialize_header_section(buf: &[u8]) -> DNSQueryHeaderSection {
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

    fn deserialize_question_section(
        buf: &[u8],
        questions_count: u16,
    ) -> (Vec<DNSQuestionQuery>, usize) {
        let mut queries = Vec::new();
        let mut index = 96 / 8;
        for _ in 0..questions_count {
            let mut labels = Vec::new();
            loop {
                let octet_length = buf[index];
                if octet_length == 0 {
                    if index == 96 / 8 {
                        labels.push(".");
                    }
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
                qtype: QType::decimal_to_qtype(qtype_u16).unwrap(),
                qclass: QClass::IN,
            };
            queries.push(query);

            // move index by four octets: two octets for type and two for class.
            index += 5;
        }

        (queries, index)
    }

    fn deserialize_resource_records(
        buf: &[u8],
        mut index: usize,
        records_count: u16,
    ) -> (Vec<ResourceRecord>, usize) {
        if records_count == 0 {
            return (vec![], index);
        }

        let mut records = Vec::with_capacity(records_count as usize);

        // parse name.
        for _ in 0..records_count {
            // parse name
            let (name, updated_index) = read_labels(&buf, index);
            index = updated_index;

            // type.
            let type_code: u16 = convert_slice_to_u16(&buf[index..index + 2]);
            index += 2;

            // class.
            let class_code: u16 = convert_slice_to_u16(&buf[index..index + 2]);
            index += 2;

            // ttl.
            let ttl: u32 = convert_slice_to_u32(&buf[index..index + 4]);
            index += 4;

            // rdlength
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
                6 => {
                    let data = SOAData::deserialize(buf, index, rd_length);
                    Type::SOA(data)
                }
                12 => {
                    let (ptr_dname, _) = read_labels(buf, index);
                    Type::PTR(ptr_dname)
                }
                // 15 => Type::MX,
                16 => Type::TXT(TXTData {
                    length: rd_length as u8 - 1,
                    data: read_txt(&rd_data[1..]),
                }),
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
                _ => panic!("deserialize: type code not supported: "),
            };
            let answer = ResourceRecord {
                name,
                r#type: type_with_data,
                class: Class::to_class(class_code).unwrap(),
                ttl,
                rd_length,
            };
            records.push(answer);
            index += rd_length as usize;
        }

        (records, index)
    }
}

fn read_txt(buf: &[u8]) -> String {
    String::from(std::str::from_utf8(buf).unwrap())
}

fn write_labels(domain: &str) -> Vec<u8> {
    if domain == "." {
        return vec![0];
    }
    let labels: Vec<String> = domain
        .split('.')
        .filter(|x| x.len() > 0)
        .map(|label| label.to_string())
        .collect();
    let label_lengths: Vec<String> = domain
        .split('.')
        .filter(|x| x.len() > 0)
        .map(|label| label.len().to_string())
        .collect();
    let mut result = interleave(label_lengths, labels)
        .enumerate()
        .flat_map(|(i, val)| {
            if i % 2 == 0 {
                return vec![val.parse::<u8>().unwrap()];
            }
            val.as_bytes().to_owned()
        })
        .collect::<Vec<u8>>();
    result.append(&mut vec![0]);
    result
}

fn read_labels(buf: &[u8], offset: usize) -> (String, usize) {
    let mut index = offset;
    let mut labels = Vec::new();
    if buf[index] == 0 {
        return (".".to_string(), index + 1);
    }
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
    let mut joined_labels = labels.join(".");
    if joined_labels.len() > 1 && !joined_labels.ends_with(".") {
        joined_labels.push('.');
    }
    (joined_labels, index + 1)
}
fn get_response_code(buf: &[u8]) -> ResponseCode {
    let index_in_buf: usize = 28 / 8;
    let response_code = buf[index_in_buf] & 15;
    match response_code {
        0 => ResponseCode::NoError,
        1 => ResponseCode::FormatError,
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

fn transform_u32_to_array_of_u8(x: u32) -> [u8; 4] {
    // https://bit.ly/3k7Cxv1
    let b1: u8 = ((x >> 24) & 0xff) as u8;
    let b2: u8 = ((x >> 16) & 0xff) as u8;
    let b3: u8 = ((x >> 8) & 0xff) as u8;
    let b4: u8 = (x & 0xff) as u8;

    [b1, b2, b3, b4]
}

// Assumes i < len(buf) * 8.
fn is_ith_bit_set(buf: &[u8], i: usize) -> bool {
    let index_in_buf: usize = i / 8;
    let index_in_byte: usize = i % 8;
    (buf[index_in_buf] & (1 << (7 - index_in_byte))) != 0
}

#[cfg(test)]
mod tests {
    use super::{
        DNSQuery, DNSQueryHeaderSection, DNSQueryResponse, DNSQuestionQuery, OpCode, QClass, QType,
        ResponseCode, SOAData,
    };
    #[test]
    fn DNSQueryHeaderSection_serialize_id() {
        // Arrange
        let header_section = DNSQueryHeaderSection {
            id: 22015, // TODO: Generate a random number.

            // Flags.
            is_query: true,
            op_code: OpCode::Query,
            is_authoritative_answer: false,
            is_truncated: false,
            is_recursion_desired: true,
            is_recursion_available: false,
            response_code: ResponseCode::NoError,
            questions_count: 1,
            answers_count: 0,
            ns_rr_count: 0,
            additional_rr_count: 0,
        };
        let expected = [85, 255];

        // Act
        let actual = header_section.serialize();

        // Assert
        assert_eq!(expected[0], actual[0]);
        assert_eq!(expected[1], actual[1]);
    }

    #[test]
    fn DNSQuestionQuery_serialize() {
        // Arrange
        let query = DNSQuestionQuery {
            qname: "www.google.com".to_string(),
            qtype: QType::A,
            qclass: QClass::IN,
        };
        // 3 | w | w | w | 6 | g | o | o | g | l | e | 3 | c | o | m | 0
        let expected: Vec<u8> = vec![
            3, 119, 119, 119, 6, 103, 111, 111, 103, 108, 101, 3, 99, 111, 109, 0, // labels
            0, // A
            1, 0, // IN
            1,
        ];

        // Act
        let actual = query.serialize();

        // Assert
        assert_eq!(expected.len(), actual.len());
        for i in 0..expected.len() {
            assert_eq!(expected[i], actual[i]);
        }
    }

    #[test]
    fn DNSQueryResponse_deserialize() {
        let response = DNSQueryResponse {
            query: DNSQuery {
                header: DNSQueryHeaderSection {
                    id: 1, // 0 1

                    // [1, 001, 0, 1, 0, 0, 0000]
                    is_query: false,
                    op_code: OpCode::Query,
                    is_authoritative_answer: true,
                    is_truncated: false,
                    is_recursion_desired: true,
                    is_recursion_available: false,
                    response_code: ResponseCode::NoError,

                    questions_count: 1,
                    answers_count: 1,
                    ns_rr_count: 0,
                    additional_rr_count: 0,
                },
                questions: vec![],
                additionals: vec![],
            },
            answers: vec![],
            authority: vec![],
            additional: vec![],
        };

        let expected = vec![0, 1];
    }

    #[test]
    fn DNSQuestionQuery_serialize_root_domain() {
        // Arrange
        let query = DNSQuestionQuery {
            qname: ".".to_string(),
            qtype: QType::NS,
            qclass: QClass::IN,
        };
        let expected: Vec<u8> = vec![
            0, //
            0, 2, // NS
            0, 1, // IN
        ];

        // Act
        let actual = query.serialize();

        // Assert
        assert_eq!(expected.len(), actual.len());
        for i in 0..expected.len() {
            assert_eq!(expected[i], actual[i]);
        }
    }

    #[test]
    fn DNSQuestionQuery_serialize_leading_dot() {
        // Arrange
        let query = DNSQuestionQuery {
            qname: "www.google.com.".to_string(),
            qtype: QType::NS,
            qclass: QClass::IN,
        };
        let expected: Vec<u8> = vec![
            3, 119, 119, 119, 6, 103, 111, 111, 103, 108, 101, 3, 99, 111, 109, 0, // labels
            0, // NS
            2, 0, // IN
            1,
        ];

        // Act
        let actual = query.serialize();

        // Assert
        assert_eq!(expected.len(), actual.len());
        for i in 0..expected.len() {
            assert_eq!(expected[i], actual[i]);
        }
    }

    #[test]
    fn soa_data_serialize() {
        // Arrange
        let soa = SOAData {
            rname: "lafolle.ca".to_string(),
            mname: "lafolle.ca".to_string(),
            serial: 0,
            refresh_in_secs: 1,
            retry_in_secs: 2,
            expire_in_secs: 3,
            minimum: 200,
        };
        let expected = vec![
            7, 108, 97, 102, 111, 108, 108, 101, 2, 99, 97, 0, 7, 108, 97, 102, 111, 108, 108, 101,
            2, 99, 97, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 200,
        ];

        // Act
        let actual = soa.serialize();

        // Assert
        assert_eq!(actual, expected);
    }

    #[test]
    fn soa_data_deserialize() {
        // Arrange
        let raw = vec![
            7, 108, 97, 102, 111, 108, 108, 101, 2, 99, 97, 0, 7, 108, 97, 102, 111, 108, 108, 101,
            2, 99, 97, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 200,
        ];
        let expected = SOAData {
            rname: "lafolle.ca.".to_string(),
            mname: "lafolle.ca.".to_string(),
            serial: 0,
            refresh_in_secs: 1,
            retry_in_secs: 2,
            expire_in_secs: 3,
            minimum: 200,
        };

        // Act
        let actual = SOAData::deserialize(&raw, 0, raw.len() as u16);

        // Assert
        assert_eq!(expected, actual);
    }
}
