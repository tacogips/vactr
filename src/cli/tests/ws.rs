//! The session-socket handshake (design 14.5.10, 17): the constant-time
//! token compare, the path/token check, and the connection limit.

use crate::cli::ws::{check_request, ct_eq, ConnLimit};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn ct_eq_matches_only_equal_bytes_of_equal_length() {
    assert!(ct_eq(b"abcdef", b"abcdef"));
    assert!(!ct_eq(b"abcdef", b"abcdeg"));
    assert!(!ct_eq(b"abc", b"abcd"));
    assert!(!ct_eq(b"", b"x"));
    assert!(ct_eq(b"", b""));
}

#[test]
fn check_request_accepts_the_right_path_and_token() {
    let pq = format!("/session?token={TOKEN}");
    assert_eq!(check_request(&pq, TOKEN), Ok(()));
}

#[test]
fn check_request_accepts_extra_query_parameters() {
    let pq = format!("/session?a=1&token={TOKEN}&b=2");
    assert_eq!(check_request(&pq, TOKEN), Ok(()));
}

#[test]
fn check_request_rejects_a_wrong_path() {
    let pq = format!("/nope?token={TOKEN}");
    assert_eq!(check_request(&pq, TOKEN), Err(401));
}

#[test]
fn check_request_rejects_a_wrong_token() {
    let pq = "/session?token=wrong";
    assert_eq!(check_request(pq, TOKEN), Err(401));
}

#[test]
fn check_request_rejects_a_missing_token() {
    assert_eq!(check_request("/session", TOKEN), Err(401));
    assert_eq!(check_request("/session?other=1", TOKEN), Err(401));
}

#[test]
fn conn_limit_rejects_the_ninth_connection() {
    let limit = ConnLimit::new(8);
    for _ in 0..8 {
        assert!(limit.acquire(), "the first 8 connections must be accepted");
    }
    assert!(!limit.acquire(), "the 9th connection must be rejected");
    limit.release();
    assert!(limit.acquire(), "a released slot is available again");
}
