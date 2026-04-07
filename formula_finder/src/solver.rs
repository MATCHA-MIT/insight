use crate::predicates;
use crate::teacher;
use crate::data_types;

pub trait Solver<'a> {
    fn new_from_teacher(teacher: &'a teacher::Teacher) -> Self;

    fn debug_invariant(
        &self,
        invariant: &predicates::Invariant,
        formula_score_weights: &data_types::general_data_types::FormulaScoreWeights
    );

    fn score_invariant(     
        &self,
        invariant: &predicates::Invariant
    ) -> predicates::PriorityScores;

    fn merge_invariant_and_score(&self, left_invariant: &predicates::ScoredInvariantWithFulfilledExample, right_invariant: &predicates::ScoredInvariantWithFulfilledExample) -> predicates::ScoredInvariantWithFulfilledExample;

    fn score_invariant_with_fulfilled_examples(&self, invariant: &predicates::Invariant) -> predicates::ScoreAndFulfilledExample;

    fn score_merged_invariant(&self, left_invariant: &predicates::ScoredInvariantWithFulfilledExample, right_invariant: &predicates::ScoredInvariantWithFulfilledExample) -> predicates::ScoreAndFulfilledExample;

    fn calculate_invariant_objective(&self, invariant: &predicates::Invariant,allow_must_fullfill_bex_not_covered: bool, formula_score_weights: &data_types::general_data_types::FormulaScoreWeights) -> predicates::InvariantObjective;
}

