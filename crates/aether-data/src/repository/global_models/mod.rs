mod memory;
mod sql;

#[allow(unused_imports)]
pub(crate) use aether_data_contracts::repository::global_models::{
    AdminGlobalModelListQuery, AdminProviderModelListQuery, CreateAdminGlobalModelRecord,
    GlobalModelReadRepository, GlobalModelWriteRepository, PublicCatalogModelListQuery,
    PublicCatalogModelSearchQuery, PublicGlobalModelQuery, StoredAdminGlobalModel,
    StoredAdminGlobalModelPage, StoredAdminProviderModel, StoredProviderActiveGlobalModel,
    StoredProviderModelStats, StoredPublicCatalogModel, StoredPublicGlobalModel,
    StoredPublicGlobalModelPage, UpdateAdminGlobalModelRecord, UpsertAdminProviderModelRecord,
};
pub use memory::InMemoryGlobalModelReadRepository;
pub use sql::SqlxGlobalModelReadRepository;
