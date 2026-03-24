mod memory;
mod record;
mod sql;
mod types;

pub use memory::InMemoryShadowResultRepository;
pub use record::{merge_shadow_result_sample, RecordShadowResultSample, ShadowResultSampleOrigin};
pub use sql::SqlxShadowResultRepository;
pub use types::{
    ShadowResultLookupKey, ShadowResultMatchStatus, ShadowResultReadRepository,
    ShadowResultRepository, ShadowResultWriteRepository, StoredShadowResult, UpsertShadowResult,
};
