//! HTML building blocks and per-variant bodies of the answer pages.

use super::page::{MeshChoice, MeshPage, OpenPort};
use super::page_texts::*;

/// Status, title and inner HTML of one answer page.
pub(super) struct Parts {
    pub status: u16,
    pub title: String,
    pub body: String,
}

/// Escapes text for HTML content and attribute values.
pub(super) fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        out.push_str(&esc_char(c));
    }
    out
}

fn esc_char(c: char) -> String {
    let named = [('&', "&amp;"), ('<', "&lt;"), ('>', "&gt;"), ('"', "&quot;"), ('\'', "&#39;")];
    named
        .iter()
        .find(|(from, _)| *from == c)
        .map_or_else(|| c.to_string(), |(_, to)| (*to).to_string())
}

/// Fills `{key}` marks of a template in one pass; values are escaped, so a value
/// that itself contains `{key}` is never expanded again.
pub(super) fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else { break };
        out.push_str(&rest[..start]);
        out.push_str(&value_of(values, &rest[start + 1..start + len]));
        rest = &rest[start + len + 1..];
    }
    out.push_str(rest);
    out
}

fn value_of(values: &[(&str, &str)], key: &str) -> String {
    values.iter().find(|(k, _)| *k == key).map(|(_, v)| esc(v)).unwrap_or_default()
}

fn para(class: &str, html: &str) -> String {
    format!("<p class=\"{class}\">{html}</p>")
}

fn retry(url: &str) -> String {
    format!("<a class=\"btn\" href=\"{}\">{PAGE_TRY_AGAIN}</a>", esc(url))
}

fn copy_button(label: &str, source: &str, text: &str, fail: &str) -> String {
    format!(
        "<button type=\"button\" class=\"btn\" aria-label=\"{}\" data-copy=\"{}\" data-fail=\"{}\" data-done=\"{PAGE_COPIED}\">{text}</button>",
        esc(label),
        esc(source),
        esc(fail),
    )
}

fn who(page: &MeshPage) -> String {
    para("hint", &fill(PAGE_WHO, &[("me", page.me.as_str()), ("host", page.host.as_str())]))
}

/// h1, lead paragraph, the "who speaks" line, then the rest.
fn card(page: &MeshPage, h1: &str, lead: &str, rest: &str) -> String {
    format!("<h1>{h1}</h1>{lead}{}{rest}", who(page))
}

fn port_line(port: &OpenPort) -> String {
    let link = format!("<a href=\"{0}\">{0}</a>", esc(&port.url));
    let name = format!(" <span class=\"muted\">— {}</span>", esc(&port.name));
    format!("<li>{link}{}</li>", if port.name.is_empty() { "" } else { name.as_str() })
}

fn port_list(heading: &str, open: &[OpenPort]) -> String {
    let items: String = open.iter().map(port_line).collect();
    format!("<h2>{heading}</h2><ul>{items}</ul>")
}

fn single_port(port: &OpenPort) -> String {
    let template = if port.name.is_empty() { PAGE_OPEN_PORT } else { PAGE_OPEN_PORT_NAMED };
    let p = port.port.to_string();
    let label = fill(template, &[("p", p.as_str()), ("port_name", port.name.as_str())]);
    format!("<p><a class=\"btn btn-primary\" href=\"{}\">{label}</a></p>", esc(&port.url))
}

fn open_block(device: &str, open: &[OpenPort]) -> String {
    let values = [("name", device)];
    if open.is_empty() {
        return para("", &fill(PAGE_NO_SHARED_PORTS, &values));
    }
    if let [only] = open {
        return single_port(only);
    }
    port_list(&fill(PAGE_H2_SHARED_PORTS, &values), open)
}

fn parts(status: u16, title: String, body: String) -> Parts {
    Parts { status, title, body }
}

fn slug_texts(label: &str) -> (String, String) {
    if label.is_empty() {
        return (PAGE_TITLE_NO_NAME.to_string(), PAGE_TITLE_NO_NAME.to_string());
    }
    let values = [("slug", label)];
    (fill(PAGE_TITLE_NOT_FOUND, &values), fill(PAGE_H1_NOT_FOUND, &values))
}

fn did_you_mean(suggest: &[String]) -> String {
    if suggest.is_empty() {
        return String::new();
    }
    let links: Vec<String> =
        suggest.iter().map(|s| format!("<a href=\"{0}\">{0}</a>", esc(s))).collect();
    para("", &format!("{PAGE_DID_YOU_MEAN} {}?", links.join(PAGE_OR)))
}

pub(super) fn not_found(page: &MeshPage, label: &str, suggest: &[String]) -> Parts {
    let (title, h1) = slug_texts(label);
    let lead = para("", &esc(PAGE_BODY_NOT_FOUND)) + &did_you_mean(suggest);
    let rest = para("hint", &esc(PAGE_HINT_NOT_FOUND));
    parts(404, title, card(page, &h1, &lead, &rest))
}

fn choice_row(choice: &MeshChoice) -> String {
    let badge = if choice.online { PAGE_ONLINE } else { PAGE_OFFLINE };
    let label = fill(PAGE_COPY_ADDRESS_LABEL, &[("url", choice.url.as_str())]);
    let button = copy_button(&label, &choice.url, PAGE_COPY, PAGE_COPY_FAIL_ADDRESS);
    let name = format!("<div><strong>{}</strong><span class=\"badge\">{badge}</span></div>", esc(&choice.name));
    let link = format!("<a class=\"mono\" href=\"{0}\">{0}</a> {button}", esc(&choice.url));
    format!("<div class=\"card\">{name}<div>{link}</div></div>")
}

pub(super) fn ambiguous(page: &MeshPage, choices: &[MeshChoice]) -> Parts {
    let count = choices.len().to_string();
    let values = [("host", page.host.as_str()), ("count", count.as_str())];
    let lead = para("", &fill(PAGE_BODY_AMBIGUOUS, &values));
    let rows: String = choices.iter().map(choice_row).collect();
    let rest = rows + &para("hint", &esc(PAGE_HINT_AMBIGUOUS));
    let h1 = fill(PAGE_H1_AMBIGUOUS, &values);
    parts(300, fill(PAGE_TITLE_AMBIGUOUS, &values), card(page, &h1, &lead, &rest))
}

fn access_steps(values: &[(&str, &str)]) -> String {
    let steps = [PAGE_STEP_OPEN_TAB, PAGE_STEP_PUBLISH, PAGE_STEP_SHARE];
    let items: String = steps.iter().map(|s| format!("<li>{}</li>", fill(s, values))).collect();
    let label = fill(PAGE_COPY_MESSAGE_LABEL, values);
    let button = copy_button(&label, "#owner-message", PAGE_COPY_MESSAGE, PAGE_COPY_FAIL_TEXT);
    let message = format!("<div class=\"msg\" id=\"owner-message\">{}</div>", fill(PAGE_MESSAGE, values));
    let ask = para("", &fill(PAGE_BODY_GET_ACCESS, values));
    let after = para("", &esc(PAGE_THEN_TRY_AGAIN));
    let head = format!("<h2>{PAGE_H2_GET_ACCESS}</h2>{ask}<ol>{items}</ol>");
    format!("{head}{}{message}{button}{after}", para("", &esc(PAGE_MESSAGE_LABEL)))
}

pub(super) fn not_opened(page: &MeshPage, device: &str, url: &str, port: u16, open: &[OpenPort], offline: bool) -> Parts {
    let port = port.to_string();
    let values = [("name", device), ("port", port.as_str()), ("me", page.me.as_str()), ("host", page.host.as_str())];
    let lead = para("", &fill(PAGE_BODY_NOT_OPENED, &values));
    let also = if offline { para("", &fill(PAGE_ALSO_OFFLINE, &values)) } else { String::new() };
    let rest = open_block(device, open) + &also + &access_steps(&values) + &retry(url);
    let h1 = fill(PAGE_H1_NOT_OPENED, &values);
    parts(403, fill(PAGE_TITLE_NOT_OPENED, &values), card(page, &h1, &lead, &rest))
}

pub(super) fn offline(page: &MeshPage, device: &str, url: &str, port: u16, open: &[OpenPort]) -> Parts {
    let port = port.to_string();
    let values = [("name", device), ("port", port.as_str())];
    let lead = para("", &fill(PAGE_BODY_OFFLINE, &values));
    let others = if open.is_empty() { String::new() } else { port_list(&fill(PAGE_H2_OTHER_PORTS, &values), open) };
    let todo = para("", &fill(PAGE_BODY_OFFLINE_TODO, &values));
    let rest = format!("{others}<h2>{PAGE_H2_WHAT_TO_DO}</h2>{todo}{}", retry(url));
    let title = fill(PAGE_TITLE_OFFLINE, &values);
    parts(503, title.clone(), card(page, &title, &lead, &rest))
}

pub(super) fn no_service(page: &MeshPage, device: &str, url: &str, port: u16, timed_out: bool) -> Parts {
    let port = port.to_string();
    let values = [("name", device), ("port", port.as_str()), ("host", page.host.as_str())];
    let (status, body) = if timed_out { (504, PAGE_BODY_TIMED_OUT) } else { (502, PAGE_BODY_REFUSED) };
    let lead = para("", &fill(body, &values));
    let todo = para("", &fill(PAGE_BODY_NO_SERVICE_TODO, &values));
    let rest = format!("<h2>{PAGE_H2_WHAT_TO_DO}</h2>{todo}{}", retry(url));
    let h1 = fill(PAGE_H1_NO_SERVICE, &values);
    parts(status, fill(PAGE_TITLE_NO_SERVICE, &values), card(page, &h1, &lead, &rest))
}
