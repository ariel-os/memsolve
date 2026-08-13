//! Memory layout solver
use crate::{
    bin::Bin,
    layout::ResolvedLayout,
    section::{ResolvedSection, Section, SectionError},
};
use conv::{ApproxInto, ValueInto};
use itertools::Itertools;
use microlp::{ComparisonOp, LinearExpr, OptimizationDirection, Problem, SolveOutcome};
use thiserror::Error;

#[derive(Error, Clone, Debug, PartialEq)]
pub(super) enum SolverError {
    #[error("solver error: {0}")]
    Solver(#[from] microlp::Error),
    #[error("too many pages in the chip flash for solver")]
    TooManyFlashPages,
    #[error("no free regions")]
    NoAllocationRegions,
    #[error("section error: {0}")]
    SectionError(#[from] SectionError),
    #[error("time exceeded")]
    TimeExceeded,

    #[error("conversion from float to integer failed")]
    ConversionError,
}

pub(crate) fn solve<'a, MetaData: Clone + 'a>(
    bins: &Bin,
    sections: &(impl Iterator<Item = &'a Section<MetaData>> + Clone),
) -> Result<ResolvedLayout<MetaData>, SolverError> {
    let mut problem = Problem::new(OptimizationDirection::Minimize);
    problem.set_time_limit(std::time::Duration::from_millis(100));
    let mut address_map = AddressMap::empty();
    if bins.is_empty() {
        return Err(SolverError::NoAllocationRegions);
    }

    for section in sections.clone() {
        let mut options = Vec::new();
        if let Some(bin) = bins.first()
            && section.boot
        {
            let var = problem.add_binary_var(into_f64(bin.start_address)?);
            options.push(var);
            let pages = section.required_pages_in_bin(bin).unwrap_or(0);
            let size = pages * bin.page_size;
            address_map.insert(
                bin.start_address,
                AddressOption::new(section, bin.start_address + size, pages, var),
            );
        } else {
            for bin in bins.iter() {
                let alignment = section.address_align.unwrap_or(bin.page_size);
                let mut address_option = bin.start_address.next_multiple_of(alignment);
                'inner: while address_option < bin.end_address {
                    if bin.section_fits_at_address(address_option, section) {
                        if section.address.is_none_or(|addr| addr == address_option) {
                            let var = problem.add_binary_var(into_f64(address_option)?);
                            options.push(var);
                            let pages = section.required_pages_in_bin(bin).unwrap_or(0);
                            let size = pages * bin.page_size;
                            address_map.insert(
                                address_option,
                                AddressOption::new(section, address_option + size, pages, var),
                            );
                        }
                        address_option = (address_option + 1).next_multiple_of(alignment);
                    } else {
                        break 'inner;
                    }
                }
            }
        }

        // Section must be allocated at exactly 1 address
        let mut expr = LinearExpr::empty();
        for var in options {
            expr.add(var, 1.0);
        }
        problem.add_constraint(expr, ComparisonOp::Eq, 1.0);
    }

    assert!(address_map.is_sorted_by_key(|s| s.address));

    for address in address_map.iter() {
        // Only 1 section per address
        address.restrict_to_section(&mut problem);

        for allocation in &address.allocations {
            // Checks which other potential allocations clash with this one and constrains the solver
            // on this
            let expr_components = address_map
                .iter()
                .filter_map(|a| {
                    if a.within_section(address.address, allocation) {
                        return Some(a.allocations.iter().filter_map(|o| {
                            if std::ptr::eq(o.section, allocation.section) {
                                None
                            } else {
                                Some((o.var, 1.0))
                            }
                        }));
                    }
                    None
                })
                .flatten();
            let elements = expr_components.clone().count();
            if elements > 0 {
                let constraint = expr_components
                    .into_iter()
                    .chain([(allocation.var, into_f64(elements + 1)?)]);
                problem.add_constraint(constraint, ComparisonOp::Le, into_f64(elements + 1)?);
            }
        }
    }

    let SolveOutcome::Solution(solution) = problem.solve()? else {
        return Err(SolverError::TimeExceeded);
    };

    let resolved = address_map.iter().flat_map(|address| {
        address.allocations.iter().filter_map(|option| {
            if solution.var_value(option.var) > 0.0 {
                let section = option.section;
                Some(ResolvedSection::new(
                    section.name.clone(),
                    option.pages,
                    option.end_address.saturating_sub(address.address),
                    address.address,
                    section.linker_name.clone(),
                    section.metadata.clone(),
                ))
            } else {
                None
            }
        })
    });

    Ok(resolved.collect::<Vec<_>>().into())
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
    for [(i, sec1), (j, sec2)] in sections.clone().enumerate().array_combinations() {
        let diff = sec1.relative_pages - sec2.relative_pages;
        let mut relative_constraint = LinearExpr::empty();
        relative_constraint.add(variables[i], 1.0);
        relative_constraint.add(variables[j], -1.0);
        problem.add_constraint(relative_constraint, ComparisonOp::Eq, into_f64(diff)?);
    }
    let solution = problem.solve()?;
    let SolveOutcome::Solution(solution) = solution else {
        return Err(SolverError::TimeExceeded);
    };

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

struct AddressMap<'a, MetaData: Clone>(Vec<AddressAlloc<'a, MetaData>>);

impl<'a, MetaData: Clone> AddressMap<'a, MetaData> {
    fn empty() -> Self {
        Self(Vec::new())
    }

    fn address_sort(&mut self) {
        self.sort_by_key(|element| element.address);
    }

    fn insert(&mut self, address: u64, option: AddressOption<'a, MetaData>) {
        let element = self.iter_mut().find(|e| e.address == address);
        let element = match element {
            Some(e) => e,
            None => self.0.insert_mut(0, AddressAlloc::empty(address)),
        };
        element.extend(option);
        self.address_sort();
    }
}

impl<'a, MetaData: Clone> std::ops::Deref for AddressMap<'a, MetaData> {
    type Target = Vec<AddressAlloc<'a, MetaData>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<MetaData: Clone> std::ops::DerefMut for AddressMap<'_, MetaData> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

struct AddressAlloc<'a, MetaData: Clone> {
    address: u64,
    allocations: Vec<AddressOption<'a, MetaData>>,
}

impl<'a, MetaData: Clone> AddressAlloc<'a, MetaData> {
    fn empty(address: u64) -> Self {
        Self {
            address,
            allocations: Vec::new(),
        }
    }

    fn extend(&mut self, option: AddressOption<'a, MetaData>) {
        self.allocations.push(option);
    }

    /// returns true if this allocation is within the section if that section is allocated at
    /// the address
    fn within_section(&self, address: u64, option: &AddressOption<'a, MetaData>) -> bool {
        self.address > address && self.address < option.end_address
    }

    fn restrict_to_section(&self, problem: &mut microlp::Problem) {
        if self.allocations.len() > 1 {
            problem.add_constraint(
                self.allocations.iter().map(|a| (a.var, 1.0)),
                ComparisonOp::Le,
                1.0,
            );
        }
    }
}

/// A location where a section could be allocated at
struct AddressOption<'a, MetaData: Clone> {
    section: &'a Section<MetaData>,
    end_address: u64,
    pages: u64,
    var: microlp::Variable,
}

impl<'a, MetaData: Clone> AddressOption<'a, MetaData> {
    fn new(
        section: &'a Section<MetaData>,
        end_address: u64,
        pages: u64,
        var: microlp::Variable,
    ) -> Self {
        Self {
            section,
            end_address,
            pages,
            var,
        }
    }
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
