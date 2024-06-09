use aleph_bft::DataProvider as DataProviderT;
use madara_runtime::Header;
use sp_core::H256;

#[derive(Debug, Clone)]
pub struct DataProvider {
    pub head: Header,
    pub tail: Vec<H256>,
}

impl DataProvider {
    pub fn verify_header() -> Header {
        todo!()
    }
}

#[async_trait::async_trait]
impl DataProviderT<Header> for DataProvider {
    async fn get_data(&mut self) -> Option<Header> {
        todo!()
    }
}
