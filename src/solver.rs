//! Memory layout solver
use itertools::Itertools;
use microlp::{ComparisonOp, Error as SolverError, LinearExpr, OptimizationDirection, Problem};
use uom::si::information::byte;

use crate::{
    bin::Bin,
    section::{ResolvedLayout, ResolvedSection, Section},
};

pub(crate) fn solve<'a>(
    bins: &Bin,
    sections: &(impl Iterator<Item = &'a Section> + Clone),
) -> Result<ResolvedLayout, SolverError> {
    let mut problem = Problem::new(OptimizationDirection::Minimize);
    let num_bins = bins.len();
    let largest_bin = bins.iter().max_by_key(|b| b.num_pages()).unwrap();

    let mut variables = vec![vec![]; num_bins];

    for (i, bin) in bins.iter().enumerate() {
        let mut bin_constraint = LinearExpr::empty();
        for section in sections.clone() {
            let var = problem.add_binary_var(0.0);
            variables[i].push(var);
            bin_constraint.add(var, section.pages.unwrap() as f64);
        }
        if bin == largest_bin {
            problem.add_constraint(bin_constraint, ComparisonOp::Le, bin.num_pages() as f64);
        } else {
            let free_space = problem.add_integer_var(1.0 * i as f64, (0, bin.num_pages() as i32));
            bin_constraint.add(free_space, 1.0);
            problem.add_constraint(bin_constraint, ComparisonOp::Eq, bin.num_pages() as f64);
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

    let resolved: Vec<(&crate::bin::MemoryBin, Vec<&Section>)> = bins
        .iter()
        .enumerate()
        .map(|(i, bin)| {
            let mut placed_sections: Vec<&Section> = Vec::new();
            for (j, section) in sections.clone().enumerate() {
                if solution.var_value(variables[i][j]) > 0.0 {
                    placed_sections.push(section);
                }
            }
            placed_sections.sort_by(|a, b| {
                if a.boot || b.boot {
                    b.boot.cmp(&a.boot)
                } else {
                    a.pages.unwrap().cmp(&b.pages.unwrap())
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
                    let set_pages = section.pages.unwrap();
                    let r = ResolvedSection::new(
                        section.name.clone(),
                        set_pages,
                        bin.page_size * set_pages,
                        *next_address,
                        section.section_name.clone(),
                    );
                    *next_address += bin.page_size.get::<byte>() * section.pages.unwrap();
                    Some(r)
                })
        })
        .collect();

    Ok(resolved.into())
}

pub(crate) fn solve_free<'a>(
    sections: &(impl Iterator<Item = &'a Section> + Clone),
    set_pages: u64,
) -> Result<Vec<Section>, SolverError> {
    let mut problem = Problem::new(OptimizationDirection::Maximize);
    let mut variables = Vec::with_capacity(sections.clone().count());

    let mut total_size_constraint = LinearExpr::empty();
    for section in sections.clone() {
        let var = problem.add_integer_var(1.0, (0, set_pages as i32));
        if let Some(pages) = section.pages {
            let mut size_constraint = LinearExpr::empty();
            size_constraint.add(var, 1.0);
            problem.add_constraint(size_constraint, ComparisonOp::Ge, pages as f64);
        }
        total_size_constraint.add(var, 1.0);
        variables.push(var);
    }
    problem.add_constraint(total_size_constraint, ComparisonOp::Le, set_pages as f64);

    for ((i, sec1), (j, sec2)) in sections.clone().enumerate().tuple_combinations() {
        let diff = sec1.relative_pages - sec2.relative_pages;
        let mut relative_constraint = LinearExpr::empty();
        relative_constraint.add(variables[i], 1.0);
        relative_constraint.add(variables[j], -1.0);
        problem.add_constraint(relative_constraint, ComparisonOp::Eq, diff as f64);
    }
    let solution = problem.solve()?;

    #[allow(clippy::cast_possible_truncation)]
    Ok(sections
        .clone()
        .enumerate()
        .map(|(i, s)| {
            let pages = solution.var_value(variables[i]) as u64;
            s.clone().set_pages(pages)
        })
        .collect())
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
}
