//! Tests of `sources`: fixtures of the three sources — the response bodies as the services return them,
//! edge cases and `read_answer`.

use super::*;

/// An `ipwho.is` answer in the service's format, every value replaced with a placeholder.
const IPWHO_OK: &str = r#"{"ip":"203.0.113.7","success":true,"type":"IPv4","continent":"Europe","continent_code":"EU","country":"Testland","country_code":"DE","region":"Testville","region_code":"BE","city":"Testville","latitude":52.52,"longitude":13.405,"is_eu":true,"postal":"10115","calling_code":"49","capital":"Testville","borders":"AT,BE,CH,CZ,DK,FR,LU,NL,PL","flag":{"img":"...","emoji":"🇩🇪","emoji_unicode":"U+1F1E9 U+1F1EA"},"connection":{"asn":64496,"org":"Example ISP","isp":"...","domain":"example.net"},"timezone":{"id":"Europe/Testville","abbr":"CET","is_dst":false,"offset":3600,"utc":"+01:00"},"readme":"https://ipwhois.io/docs"}"#;
/// `ipwho.is` answers 200 with `success: false` for an invalid IP — this is a refusal.
const IPWHO_INVALID_IP: &str =
    r#"{"ip":"999.1.1.1","success":false,"message":"Invalid IP address"}"#;
/// `ipwho.is` on `GET /bogus`: status 404.
const IPWHO_404: &str = r#"{"success":false,"message":"404 not found"}"#;
/// An `api.country.is` answer in the service's format, with placeholder values.
const COUNTRY_IS_OK: &str = r#"{"ip":"203.0.113.7","country":"DE"}"#;
/// `api.country.is` on `GET /bogus`: status 404.
const COUNTRY_IS_404: &str = r#"{"error":{"code":404,"message":"Not Found"}}"#;
/// The significant fields of a trace answer; the full body is longer, the other fields
/// do not affect parsing.
const TRACE_OK: &str = "ip=203.0.113.7\nloc=DE\ntls=TLSv1.3\nsni=off\nhttp=http/1.1\ncolo=FRA";
const TRACE_CRLF: &str =
    "ip=203.0.113.7\r\nloc=DE\r\ntls=TLSv1.3\r\nsni=off\r\nhttp=http/1.1\r\ncolo=FRA";
const TRACE_NO_LOC: &str = "ip=203.0.113.7\ntls=TLSv1.3\nsni=off\nhttp=http/1.1\ncolo=FRA";

#[test]
fn parses_ipwho() {
    assert_eq!(parse_ipwho(IPWHO_OK.as_bytes()).unwrap().as_str(), "DE");
    assert!(parse_ipwho(IPWHO_INVALID_IP.as_bytes()).is_none());
    assert!(parse_ipwho(IPWHO_404.as_bytes()).is_none());
    assert!(parse_ipwho(br#"{"success":true}"#).is_none());
    assert!(parse_ipwho(b"not json").is_none());
    assert!(parse_ipwho(b"").is_none());
}

#[test]
fn parses_country_is() {
    assert_eq!(
        parse_country_is(COUNTRY_IS_OK.as_bytes()).unwrap().as_str(),
        "DE"
    );
    assert!(parse_country_is(COUNTRY_IS_404.as_bytes()).is_none());
    assert!(parse_country_is(b"").is_none());
}

#[test]
fn parses_trace() {
    assert_eq!(parse_trace(TRACE_OK.as_bytes()).unwrap().as_str(), "DE");
    assert_eq!(parse_trace(TRACE_CRLF.as_bytes()).unwrap().as_str(), "DE");
    assert!(parse_trace(TRACE_NO_LOC.as_bytes()).is_none());
    assert!(parse_trace(b"loc=XX").is_none());
    assert!(parse_trace(b"loc=T1").is_none());
}

#[test]
fn read_answer_classifies() {
    assert_eq!(
        read_answer(Source::CountryIs, 200, COUNTRY_IS_OK.as_bytes())
            .unwrap()
            .as_str(),
        "DE"
    );
    let err = read_answer(Source::IpWhoIs, 429, IPWHO_OK.as_bytes()).unwrap_err();
    assert!(err.contains("429"), "text without 429: {err}");
    assert!(read_answer(Source::CountryIs, 200, COUNTRY_IS_404.as_bytes()).is_err());
}

#[test]
fn order_and_cf_trace_spec() {
    assert_eq!(
        SOURCE_ORDER,
        [Source::IpWhoIs, Source::CountryIs, Source::CfTrace]
    );
    let spec = Source::CfTrace.spec();
    assert_eq!(spec.host, "1.1.1.1");
    assert_eq!(spec.path, "/cdn-cgi/trace");
}
