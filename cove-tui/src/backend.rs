pub mod cove;

#[derive(Debug)]
pub enum Event {
    Cove(String, cove::conn::Event),
}
