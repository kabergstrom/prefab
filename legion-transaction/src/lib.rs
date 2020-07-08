// Stores and applies diffs to legion worlds
mod component_diffs;
pub use component_diffs::ComponentDiff;
pub use component_diffs::ComponentDiffOp;
pub use component_diffs::EntityDiff;
pub use component_diffs::EntityDiffOp;
pub use component_diffs::WorldDiff;
pub use component_diffs::apply_diff;
pub use component_diffs::apply_diff_to_prefab;
pub use component_diffs::apply_diff_to_cooked_prefab;
pub use component_diffs::ApplyDiffToPrefabError;

// Generates diffs by comparing legion worlds
mod transactions;
pub use transactions::TransactionBuilder;
pub use transactions::Transaction;
pub use transactions::TransactionDiffs;
pub use transactions::TransactionEntityInfo;

// A utility iterator that simplifies accessing values from SpawnFrom
mod option_iter;
pub use option_iter::OptionIter;
//pub use option_iter::iter_components_in_storage;
