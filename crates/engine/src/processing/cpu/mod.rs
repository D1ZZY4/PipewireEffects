#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

mod processor;

pub use processor::Processor;
