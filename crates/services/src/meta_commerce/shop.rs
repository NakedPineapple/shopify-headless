//! Meta Commerce Shop / Account info.

use tracing::instrument;

use super::MetaCommerceError;
use super::client::MetaCommerceClient;
use super::types::CommerceAccountInfo;

impl MetaCommerceClient {
    /// Get the commerce account information.
    ///
    /// Calls `GET /{commerce_account_id}` with name field.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn get_commerce_account(&self) -> Result<CommerceAccountInfo, MetaCommerceError> {
        let account_id = self.commerce_account_id().to_string();
        let path = format!("/{account_id}");

        let params = [("fields", "id,name")];

        self.execute(&path, Some(&params)).await
    }
}
