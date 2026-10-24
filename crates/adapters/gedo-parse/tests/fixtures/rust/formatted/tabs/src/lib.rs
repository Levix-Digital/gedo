//! Conformance fixture: every Rust entity kind and path rule (research R7).
#![allow(dead_code)]

use std::collections::HashMap;
use std::fmt::{self, Display};

mod a;
pub mod declared;

/// A constant with documentation.
pub const LIMIT: usize = 16;
static mut COUNTER: u32 = 0;

pub type Map = HashMap<String, u32>;

#[derive(Debug, Clone)]
pub struct Invoice {
	/// The total in cents.
	total: u64,
	lines: Vec<Line>,
}

pub struct Line(u64);

pub struct Unit;

pub enum Status {
	Draft,
	Sent { at: u64 },
	Paid(u64),
}

#[repr(C)]
pub union Bits {
	int: u32,
	float: f32,
}

pub trait Priced {
	const CURRENCY: &'static str;
	type Amount;

	fn price(&self) -> Self::Amount;

	fn describe(&self) -> String {
		String::from("priced")
	}
}

impl Invoice {
	pub const EMPTY: u64 = 0;

	pub fn new() -> Self {
		Self {
			total: 0,
			lines: Vec::new(),
		}
	}

	pub fn total(&self) -> u64 {
		fn helper(value: u64) -> u64 {
			value
		}
		struct Local;
		helper(self.total)
	}
}

impl Priced for Invoice {
	const CURRENCY: &'static str = "EUR";
	type Amount = u64;

	fn price(&self) -> u64 {
		self.total
	}
}

impl Display for Invoice {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.total)
	}
}

pub struct Stack<T> {
	items: Vec<T>,
}

impl<T> Stack<T> {
	pub fn push(&mut self, item: T) {
		self.items.push(item);
	}
}

impl Stack<u8> {
	pub fn push(&mut self, item: u8) {
		self.items.push(item);
	}
}

impl<'a, T: Clone> From<&'a [T]> for Stack<T>
where
	T: Default,
{
	fn from(items: &'a [T]) -> Self {
		Self {
			items: items.to_vec(),
		}
	}
}

#[cfg(unix)]
pub fn platform() -> &'static str {
	"unix"
}

#[cfg(not(unix))]
pub fn platform() -> &'static str {
	"other"
}

pub mod inline {
	pub mod nested {
		pub fn deep() -> u8 {
			match 1 {
				0 => 0,
				n => n + 1,
			}
		}
	}

	pub fn r#type() -> char {
		'x'
	}

	pub fn größe() -> usize {
		"a\r\nb".len()
	}
}

macro_rules! square {
	($x:expr) => {
		$x * $x
	};
}

extern "C" {
	fn abs(input: i32) -> i32;
}

pub async fn later<F: Fn() -> u8>(callback: F) -> u8 {
	let closure = |x: u8| x + 1;
	closure(callback())
}
