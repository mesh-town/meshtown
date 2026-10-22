//! Three geoip answer sources: the address, parsing the body, the query order.
//!
//! What it does: describes `ipwho.is`, `api.country.is` and Cloudflare `/cdn-cgi/trace` — where
//! to go and how to extract the country code from the answer body; classifies an HTTP answer as success/refusal.
//! What it doesn't do: doesn't go to the network — the network request is assembled by `geoip::fetch`.
//! Links: `geoip::fetch::lookup_country` goes over the sources by `SOURCE_ORDER`.

use super::country::Country;

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;

/// A geoip answer source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    IpWhoIs,
    CountryIs,
    CfTrace,
}

/// The query order: first `ipwho.is`, then `api.country.is`, then Cloudflare by IP
/// without DNS.
pub const SOURCE_ORDER: [Source; 3] = [Source::IpWhoIs, Source::CountryIs, Source::CfTrace];

/// Where to go and how to parse the body of one source.
pub struct SourceSpec {
    pub host: &'static str,
    pub path: &'static str,
    pub accept: &'static str,
    pub parse: fn(&[u8]) -> Option<Country>,
}

impl Source {
    /// The address and parsing of one source — the only `match` on `Source` in the module.
    pub fn spec(self) -> SourceSpec {
        match self {
            Source::IpWhoIs => SourceSpec {
                host: "ipwho.is",
                path: "/",
                accept: "application/json",
                parse: parse_ipwho,
            },
            Source::CountryIs => SourceSpec {
                host: "api.country.is",
                path: "/",
                accept: "application/json",
                parse: parse_country_is,
            },
            Source::CfTrace => SourceSpec {
                host: "1.1.1.1",
                path: "/cdn-cgi/trace",
                accept: "text/plain",
                parse: parse_trace,
            },
        }
    }
}

/// `ipwho.is`: JSON, valid only with `success == true` and the field `country_code`.
fn parse_ipwho(body: &[u8]) -> Option<Country> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    if !value.get("success")?.as_bool()? {
        return None;
    }
    Country::parse(value.get("country_code")?.as_str()?)
}

/// `api.country.is`: JSON, the field `country`.
fn parse_country_is(body: &[u8]) -> Option<Country> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    Country::parse(value.get("country")?.as_str()?)
}

/// Cloudflare `/cdn-cgi/trace`: `key=value` text by lines (`\n` or `\r\n`), looks for `loc=`.
fn parse_trace(body: &[u8]) -> Option<Country> {
    let text = std::str::from_utf8(body).ok()?;
    text.split(['\r', '\n'])
        .find_map(|line| line.strip_prefix("loc="))
        .and_then(Country::parse)
}

/// Classifies a source's answer: not 200 or a body without a country is a refusal with a reason.
pub fn read_answer(source: Source, status: u16, body: &[u8]) -> Result<Country, String> {
    if status != 200 {
        return Err(format!("HTTP {status}"));
    }
    (source.spec().parse)(body).ok_or_else(|| "answer without a country".to_string())
}
