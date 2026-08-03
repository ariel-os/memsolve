//! Memory layout solver
use crate::{
    bin::Bin,
    layout::ResolvedLayout,
    section::{ResolvedSection, Section, SectionError},
};
use conv::{ApproxInto, ValueInto};
use itertools::Itertools;
use microlp::{ComparisonOp, LinearExpr, OptimizationDirection, Problem};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub(super) enum SolverError {
    #[error("solver error: {0}")]
    Solver(#[from] microlp::Error),
    #[error("too many pages in section {0} for solver")]
    TooManySectionPages(Section<()>),
    #[error("too many pages in the chip flash for solver")]
    TooManyFlashPages,
    #[error("no free regions")]
    NoAllocationRegions,
    #[error("section error: {0}")]
    SectionError(#[from] SectionError),

    #[error("conversion from float to integer failed")]
    ConversionError,
}

pub(crate) fn solve<'a, MetaData: Clone + 'a>(
    bins: &Bin,
    sections: &(impl Iterator<Item = &'a Section<MetaData>> + Clone),
) -> Result<ResolvedLayout<MetaData>, SolverError> {
    let mut problem = Problem::new(OptimizationDirection::Minimize);
    let num_bins = bins.len();
    let largest_bin = bins.largest_bin().ok_or(SolverError::NoAllocationRegions)?;

    let mut variables = vec![vec![]; num_bins];

    for (i, bin) in bins.iter().enumerate() {
        let mut bin_constraint = LinearExpr::empty();
        for section in sections.clone() {
            let var = problem.add_binary_var(0.0);
            variables[i].push(var);
            bin_constraint.add(
                var,
                section
                    .required_pages_in_bin(bin)?
                    .value_into()
                    .map_err(|_| {
                        SolverError::TooManySectionPages(section.clone().clear_metadata())
                    })?,
            );
        }
        if bin == largest_bin {
            problem.add_constraint(bin_constraint, ComparisonOp::Le, into_f64(bin.num_pages())?);
        } else {
            let free_space = problem.add_integer_var(
                1.0 * into_f64(i)?,
                (
                    0,
                    i32::try_from(bin.num_pages()).map_err(|_| SolverError::TooManyFlashPages)?,
                ),
            );
            bin_constraint.add(free_space, 1.0);
            problem.add_constraint(
                bin_constraint,
                ComparisonOp::Eq,
                bin.num_pages_f64().ok_or(SolverError::TooManyFlashPages)?,
            );
        }
    }

    // section can be only in a single bin
    for (j, section) in sections.clone().enumerate() {
        let mut section_constraint = LinearExpr::empty();
        if section.boot {
            section_constraint.add(variables[0][j], 1.0);
        } else {
            for (i, _) in bins.iter().enumerate() {
                section_constraint.add(variables[i][j], 1.0);
            }
        }
        problem.add_constraint(section_constraint, ComparisonOp::Eq, 1.0);
    }

    let solution = problem.solve()?;

    let resolved: Vec<(&crate::bin::MemoryBin, Vec<&Section<_>>)> = bins
        .iter()
        .enumerate()
        .map(|(i, bin)| {
            let mut placed_sections: Vec<&Section<_>> = Vec::new();
            for (j, section) in sections.clone().enumerate() {
                if solution.var_value(variables[i][j]) > 0.0 {
                    placed_sections.push(section);
                }
            }
            placed_sections.sort_by(|a, b| {
                if a.boot || b.boot {
                    b.boot.cmp(&a.boot)
                } else {
                    a.required_pages_in_bin(bin)
                        .unwrap_or(0)
                        .cmp(&b.required_pages_in_bin(bin).unwrap_or(0))
                }
            });
            (bin, placed_sections)
        })
        .collect();
    let resolved: Vec<_> = resolved
        .iter()
        .flat_map(|(bin, sections)| {
            sections
                .iter()
                .scan(bin.start_address, |next_address, section| {
                    let set_pages = section.required_pages_in_bin(bin).unwrap_or(0);
                    let r = ResolvedSection::new(
                        section.name.clone(),
                        set_pages,
                        bin.page_size * set_pages,
                        *next_address,
                        section.linker_name.clone(),
                        section.metadata.clone(),
                    );
                    *next_address += bin.page_size * set_pages;
                    Some(r)
                })
        })
        .collect();

    Ok(resolved.into())
}

pub(crate) fn solve_free<'a, MetaData: Clone + 'a>(
    sections: &(impl Iterator<Item = &'a Section<MetaData>> + Clone),
    set_pages: u64,
) -> Result<Vec<Section<MetaData>>, SolverError> {
    let mut problem = Problem::new(OptimizationDirection::Maximize);
    let mut variables = Vec::with_capacity(sections.clone().count());

    let mut total_size_constraint = LinearExpr::empty();
    for section in sections.clone() {
        // Create variables for every section
        let var = problem.add_integer_var(
            1.0,
            (
                0,
                i32::try_from(set_pages).map_err(|_| SolverError::TooManyFlashPages)?,
            ),
        );
        let mut size_constraint = LinearExpr::empty();
        size_constraint.add(var, 1.0);
        // If sections have a minimum number of pages, set a minimum constraint on that, else at
        // least 1 page.
        if let Some(pages) = section.pages {
            problem.add_constraint(size_constraint, ComparisonOp::Ge, into_f64(pages)?);
        } else {
            problem.add_constraint(size_constraint, ComparisonOp::Ge, 1.0);
        }
        total_size_constraint.add(var, 1.0);
        variables.push(var);
    }
    // Add a constraint for the maximum number of pages
    problem.add_constraint(
        total_size_constraint,
        ComparisonOp::Le,
        into_f64(set_pages)?,
    );

    // Build the relative page size constraint between every section
    for ((i, sec1), (j, sec2)) in sections.clone().enumerate().tuple_combinations() {
        let diff = sec1.relative_pages - sec2.relative_pages;
        let mut relative_constraint = LinearExpr::empty();
        relative_constraint.add(variables[i], 1.0);
        relative_constraint.add(variables[j], -1.0);
        problem.add_constraint(relative_constraint, ComparisonOp::Eq, into_f64(diff)?);
    }
    let solution = problem.solve()?;

    sections
        .clone()
        .enumerate()
        .map(|(i, s)| {
            <f64 as ApproxInto<u64>>::approx_into(solution.var_value(variables[i]))
                .map_err(|_| SolverError::ConversionError)
                .map(|pages| s.clone().set_pages(pages))
        })
        .collect::<Result<Vec<Section<MetaData>>, _>>()
}

fn into_f64(value: impl ValueInto<f64>) -> Result<f64, SolverError> {
    value
        .value_into()
        .map_err(|_| SolverError::TooManyFlashPages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_maximizing() {
        let sections = (0..5)
            .map(|i| Section::new(format!("test{i}")).unwrap().set_maximize(true))
            .collect::<Vec<_>>();
        let solved = solve_free(&sections.iter(), 5).unwrap();
        for s in solved {
            assert_eq!(s.pages, Some(1));
        }
    }

    #[test]
    fn test_diff_maximizing() {
        let sections = (0..5)
            .map(|i| {
                Section::new(format!("test{i}"))
                    .unwrap()
                    .set_maximize(true)
                    .set_relative_pages(-2 + i)
            })
            .collect::<Vec<_>>();
        let solved = solve_free(&sections.iter(), 15).unwrap();
        for (i, s) in solved.iter().enumerate() {
            assert_eq!(s.pages, Some(1 + i as u64));
        }
    }

    #[test]
    fn test_zero_size() {
        let sections = (0..5)
            .map(|i| {
                Section::new(format!("test{i}"))
                    .unwrap()
                    .set_maximize(true)
                    .set_relative_pages(-2 + i)
            })
            .collect::<Vec<_>>();
        let solved = solve_free(&sections.iter(), 9).unwrap_err();
        assert_eq!(solved, SolverError::Solver(microlp::Error::Infeasible));
    }
}
