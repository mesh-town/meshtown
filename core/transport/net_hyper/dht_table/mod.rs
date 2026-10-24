//! The saved table of DHT nodes: the file format (`file`) and the periodic writer (`saver`).

mod file;
mod saver;

pub(crate) use file::{load, TTL};
pub(crate) use saver::TableSaver;

#[cfg(test)]
pub(crate) use file::{parse, render};
#[cfg(test)]
mod transport_tests;
