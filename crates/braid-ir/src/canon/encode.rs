//! Canonical encoder for Braid values.

use super::key_cmp;
use crate::value::Value;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

fn put_head_inline(out: &mut Vec<u8>, major_shifted: u8, n: u64) {
    out.push(major_shifted | n as u8);
}

fn put_head_u8(out: &mut Vec<u8>, major_shifted: u8, n: u64) {
    out.push(major_shifted | 24);
    out.push(n as u8);
}

fn put_head_u16(out: &mut Vec<u8>, major_shifted: u8, n: u64) {
    out.push(major_shifted | 25);
    out.extend_from_slice(&(n as u16).to_be_bytes());
}

fn put_head_u32(out: &mut Vec<u8>, major_shifted: u8, n: u64) {
    out.push(major_shifted | 26);
    out.extend_from_slice(&(n as u32).to_be_bytes());
}

fn put_head_u64(out: &mut Vec<u8>, major_shifted: u8, n: u64) {
    out.push(major_shifted | 27);
    out.extend_from_slice(&n.to_be_bytes());
}

pub(crate) fn put_head(out: &mut Vec<u8>, major: u8, n: u64) {
    let major_shifted = major << 5;
    match n {
        0..=23 => put_head_inline(out, major_shifted, n),
        24..=0xff => put_head_u8(out, major_shifted, n),
        0x100..=0xffff => put_head_u16(out, major_shifted, n),
        0x1_0000..=0xffff_ffff => put_head_u32(out, major_shifted, n),
        _ => put_head_u64(out, major_shifted, n),
    }
}

fn encode_int(int_val: i64, out: &mut Vec<u8>) {
    if int_val >= 0 {
        put_head(out, 0, int_val as u64);
    } else {
        put_head(out, 1, (-1i128 - int_val as i128) as u64);
    }
}

fn encode_bytes(byte_slice: &[u8], out: &mut Vec<u8>) {
    put_head(out, 2, byte_slice.len() as u64);
    out.extend_from_slice(byte_slice);
}

fn encode_text(text_str: &str, out: &mut Vec<u8>) {
    put_head(out, 3, text_str.len() as u64);
    out.extend_from_slice(text_str.as_bytes());
}

fn encode_list(items: &[Value], out: &mut Vec<u8>) {
    put_head(out, 4, items.len() as u64);
    for item in items {
        encode_into(item, out);
    }
}

fn encode_map(map_entries: &BTreeMap<String, Value>, out: &mut Vec<u8>) {
    put_head(out, 5, map_entries.len() as u64);
    let mut keys: Vec<&String> = map_entries.keys().collect();
    keys.sort_by(|a, b| key_cmp(a, b));
    for key_str in keys {
        encode_text(key_str, out);
        encode_into(&map_entries[key_str], out);
    }
}

pub(crate) fn encode_into(val: &Value, out: &mut Vec<u8>) {
    match val {
        Value::Bool(false) => out.push(0xf4),
        Value::Bool(true) => out.push(0xf5),
        Value::Int(int_val) => encode_int(*int_val, out),
        Value::Bytes(byte_slice) => encode_bytes(byte_slice, out),
        Value::Text(text_str) => encode_text(text_str, out),
        Value::List(items) => encode_list(items, out),
        Value::Map(map_entries) => encode_map(map_entries, out),
    }
}

/// Encode a value to its single canonical byte form.
pub fn encode(val: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into(val, &mut out);
    out
}
