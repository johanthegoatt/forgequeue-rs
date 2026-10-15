use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub name: String,
    pub business_value: f64,
    pub time_criticality: f64,
    pub risk_reduction: f64,
    pub effort: f64,
    /// Names of tasks that must ship first. A name that is not in the input
    /// is treated as already done.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RankedTask {
    pub task: Task,
    pub score: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BatchPlan {
    pub budget: f64,
    pub used_effort: f64,
    pub skipped_tasks: usize,
    pub selected: Vec<RankedTask>,
    /// Tasks that had room in the budget but wait on something not in the batch.
    #[serde(default)]
    pub blocked: Vec<String>,
}

/// Checks a task list before it is scored. NaN compares false against every
/// score, so one bad row used to scramble the whole ranking without a word,
/// and negative scores were dropped by the default minimum score of 0.
pub fn validate_tasks(tasks: &[Task]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for (index, task) in tasks.iter().enumerate() {
        let name = task.name.trim();
        if name.is_empty() {
            return Err(format!("Task {} has no name.", index + 1));
        }
        if !seen.insert(name.to_lowercase()) {
            return Err(format!("Task name '{name}' is used twice."));
        }
        for (field, value) in [
            ("business_value", task.business_value),
            ("time_criticality", task.time_criticality),
            ("risk_reduction", task.risk_reduction),
            ("effort", task.effort),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!(
                    "Task '{name}': {field} must be a number of 0 or more, got {value}."
                ));
            }
        }
    }
    check_dependency_cycles(tasks)
}

// Depth-first search for a cycle; a loop of tasks could never be started.
fn check_dependency_cycles(tasks: &[Task]) -> Result<(), String> {
    use std::collections::HashMap;
    let by_name: HashMap<String, &Task> = tasks
        .iter()
        .map(|t| (t.name.trim().to_lowercase(), t))
        .collect();
    // 0 = not seen, 1 = on the current path, 2 = done.
    let mut mark: HashMap<String, u8> = HashMap::new();

    fn visit(
        key: &str,
        by_name: &HashMap<String, &Task>,
        mark: &mut HashMap<String, u8>,
        path: &mut Vec<String>,
    ) -> Result<(), String> {
        match mark.get(key).copied().unwrap_or(0) {
            2 => return Ok(()),
            1 => {
                path.push(key.to_string());
                return Err(format!(
                    "Tasks depend on each other in a loop: {}.",
                    path.join(" -> ")
                ));
            }
            _ => {}
        }
        let Some(task) = by_name.get(key) else {
            return Ok(());
        };
        mark.insert(key.to_string(), 1);
        path.push(key.to_string());
        for dep in &task.depends_on {
            visit(&dep.trim().to_lowercase(), by_name, mark, path)?;
        }
        path.pop();
        mark.insert(key.to_string(), 2);
        Ok(())
    }

    for task in tasks {
        visit(
            &task.name.trim().to_lowercase(),
            &by_name,
            &mut mark,
            &mut Vec::new(),
        )?;
    }
    Ok(())
}

fn normalized_effort(value: f64) -> f64 {
    if value < 0.25 {
        0.25
    } else {
        value
    }
}

pub fn wsjf(task: &Task) -> f64 {
    let effort = normalized_effort(task.effort);
    (task.business_value + task.time_criticality + task.risk_reduction) / effort
}

pub fn prioritize(tasks: &[Task]) -> Vec<RankedTask> {
    let mut ranked: Vec<RankedTask> = tasks
        .iter()
        .cloned()
        .map(|task| RankedTask {
            score: wsjf(&task),
            task,
        })
        .collect();

    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.task.name.cmp(&b.task.name))
    });
    ranked
}

pub fn prioritize_with_filters(tasks: &[Task], min_score: f64, top: usize) -> Vec<RankedTask> {
    let ranked = prioritize(tasks);
    let mut filtered: Vec<RankedTask> = ranked
        .into_iter()
        .filter(|item| item.score >= min_score)
        .collect();
    if top > 0 && filtered.len() > top {
        filtered.truncate(top);
    }
    filtered
}

pub fn select_batch(tasks: &[Task], budget: f64) -> (Vec<RankedTask>, f64) {
    let plan = select_batch_detailed(tasks, budget);
    (plan.selected, plan.used_effort)
}

fn select_from_ranked(ranked: Vec<RankedTask>, all_tasks: &[Task], budget: f64) -> BatchPlan {
    use std::collections::HashSet;
    let safe_budget = budget.max(0.0);
    let known: HashSet<String> = all_tasks
        .iter()
        .map(|t| t.name.trim().to_lowercase())
        .collect();
    let mut done: HashSet<String> = HashSet::new();
    let mut taken = vec![false; ranked.len()];
    let mut used = 0.0;
    let index_of: std::collections::HashMap<String, usize> = ranked
        .iter()
        .enumerate()
        .map(|(i, r)| (r.task.name.trim().to_lowercase(), i))
        .collect();

    // In rank order, each task comes with whatever it still waits on: a
    // high-value task pulls its prerequisites forward and the bundle goes in
    // whole or not at all, so cheap lower-ranked work can't crowd it out.
    for i in 0..ranked.len() {
        if taken[i] {
            continue;
        }
        let mut bundle = Vec::new();
        let mut stack = vec![i];
        let mut satisfiable = true;
        while let Some(j) = stack.pop() {
            if taken[j] || bundle.contains(&j) {
                continue;
            }
            bundle.push(j);
            for dep in &ranked[j].task.depends_on {
                let key = dep.trim().to_lowercase();
                if !known.contains(&key) || done.contains(&key) {
                    continue;
                }
                // Known but filtered out (min score, top N): it can't be in this batch.
                match index_of.get(&key) {
                    Some(&k) => stack.push(k),
                    None => satisfiable = false,
                }
            }
        }
        let effort: f64 = bundle
            .iter()
            .map(|&j| normalized_effort(ranked[j].task.effort))
            .sum();
        if satisfiable && used + effort <= safe_budget {
            used += effort;
            for &j in &bundle {
                taken[j] = true;
                done.insert(ranked[j].task.name.trim().to_lowercase());
            }
        }
    }

    let mut selected = Vec::new();
    let mut blocked = Vec::new();
    let mut skipped = 0;
    for (i, item) in ranked.into_iter().enumerate() {
        if taken[i] {
            selected.push(item);
            continue;
        }
        skipped += 1;
        // It would fit on its own, so what keeps it out is what it waits on.
        if !item.task.depends_on.is_empty()
            && used + normalized_effort(item.task.effort) <= safe_budget
        {
            blocked.push(item.task.name.clone());
        }
    }

    BatchPlan {
        budget: safe_budget,
        used_effort: used,
        skipped_tasks: skipped,
        selected,
        blocked,
    }
}

pub fn select_batch_detailed(tasks: &[Task], budget: f64) -> BatchPlan {
    let ranked = prioritize(tasks);
    select_from_ranked(ranked, tasks, budget)
}

pub fn select_batch_detailed_filtered(
    tasks: &[Task],
    budget: f64,
    min_score: f64,
    top: usize,
) -> BatchPlan {
    let ranked = prioritize_with_filters(tasks, min_score, top);
    select_from_ranked(ranked, tasks, budget)
}

/// The modified Fibonacci scale SAFe uses for relative WSJF estimates.
pub const RELATIVE_SCALE: [f64; 7] = [1.0, 2.0, 3.0, 5.0, 8.0, 13.0, 20.0];

fn snap_to_scale(value: f64) -> f64 {
    RELATIVE_SCALE
        .iter()
        .copied()
        .min_by(|a, b| (a - value).abs().total_cmp(&(b - value).abs()))
        .unwrap_or(1.0)
}

fn relative_column(tasks: &[Task], pick: fn(&Task) -> f64) -> Vec<f64> {
    let smallest = tasks.iter().map(pick).fold(f64::INFINITY, f64::min);
    // Shift so the smallest item sits at 1 even when raw scores include 0.
    let offset = if smallest > 0.0 { 0.0 } else { 1.0 - smallest };
    let anchor = smallest + offset;
    tasks
        .iter()
        .map(|task| snap_to_scale((pick(task) + offset) / anchor))
        .collect()
}

/// Rewrites the three cost-of-delay components as relative estimates, the
/// way SAFe runs WSJF: one column at a time, the smallest item becomes 1 and
/// every other item is sized against it on the modified Fibonacci scale.
/// Raw scores from different people drift in absolute terms; ratios to a
/// shared anchor do not, so the ranking stops rewarding whoever scored high.
/// Effort stays in real units so the budget keeps its meaning.
pub fn to_relative_scale(tasks: &[Task]) -> Vec<Task> {
    if tasks.is_empty() {
        return Vec::new();
    }
    let value = relative_column(tasks, |t| t.business_value);
    let time = relative_column(tasks, |t| t.time_criticality);
    let risk = relative_column(tasks, |t| t.risk_reduction);
    tasks
        .iter()
        .enumerate()
        .map(|(i, task)| Task {
            business_value: value[i],
            time_criticality: time[i],
            risk_reduction: risk[i],
            ..task.clone()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        prioritize, prioritize_with_filters, select_batch, select_batch_detailed,
        to_relative_scale, validate_tasks, wsjf, Task,
    };

    #[test]
    fn prioritize_orders_by_highest_score_first() {
        let tasks = vec![
            Task {
                name: "Low".to_string(),
                business_value: 3.0,
                time_criticality: 2.0,
                risk_reduction: 2.0,
                effort: 4.0,
                depends_on: vec![],
            },
            Task {
                name: "High".to_string(),
                business_value: 9.0,
                time_criticality: 9.0,
                risk_reduction: 7.0,
                effort: 3.0,
                depends_on: vec![],
            },
        ];

        let ranked = prioritize(&tasks);
        assert_eq!(ranked[0].task.name, "High");
    }

    #[test]
    fn prioritize_uses_name_as_tie_breaker() {
        let tasks = vec![
            Task {
                name: "Zeta".to_string(),
                business_value: 5.0,
                time_criticality: 5.0,
                risk_reduction: 5.0,
                effort: 3.0,
                depends_on: vec![],
            },
            Task {
                name: "Alpha".to_string(),
                business_value: 5.0,
                time_criticality: 5.0,
                risk_reduction: 5.0,
                effort: 3.0,
                depends_on: vec![],
            },
        ];

        let ranked = prioritize(&tasks);
        assert_eq!(ranked[0].task.name, "Alpha");
    }

    #[test]
    fn wsjf_clamps_tiny_effort_values() {
        let task = Task {
            name: "Tiny effort".to_string(),
            business_value: 5.0,
            time_criticality: 5.0,
            risk_reduction: 5.0,
            effort: 0.01,
            depends_on: vec![],
        };

        let score = wsjf(&task);
        assert_eq!(score, 60.0);
    }

    #[test]
    fn prioritize_with_filters_applies_threshold_and_limit() {
        let tasks = vec![
            Task {
                name: "A".to_string(),
                business_value: 8.0,
                time_criticality: 8.0,
                risk_reduction: 8.0,
                effort: 2.0,
                depends_on: vec![],
            },
            Task {
                name: "B".to_string(),
                business_value: 3.0,
                time_criticality: 3.0,
                risk_reduction: 3.0,
                effort: 2.0,
                depends_on: vec![],
            },
            Task {
                name: "C".to_string(),
                business_value: 7.0,
                time_criticality: 7.0,
                risk_reduction: 6.0,
                effort: 2.0,
                depends_on: vec![],
            },
        ];

        let filtered = prioritize_with_filters(&tasks, 8.0, 1);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].score >= 8.0);
    }

    #[test]
    fn select_batch_stays_under_budget() {
        let tasks = vec![
            Task {
                name: "A".to_string(),
                business_value: 8.0,
                time_criticality: 7.0,
                risk_reduction: 6.0,
                effort: 4.0,
                depends_on: vec![],
            },
            Task {
                name: "B".to_string(),
                business_value: 8.0,
                time_criticality: 8.0,
                risk_reduction: 8.0,
                effort: 5.0,
                depends_on: vec![],
            },
            Task {
                name: "C".to_string(),
                business_value: 6.0,
                time_criticality: 6.0,
                risk_reduction: 6.0,
                effort: 3.0,
                depends_on: vec![],
            },
        ];

        let (_plan, used) = select_batch(&tasks, 7.0);
        assert!(used <= 7.0);
    }

    fn task(name: &str, value: f64, time: f64, risk: f64, effort: f64) -> Task {
        Task {
            name: name.to_string(),
            business_value: value,
            time_criticality: time,
            risk_reduction: risk,
            effort,
            depends_on: vec![],
        }
    }

    #[test]
    fn relative_scale_anchors_smallest_item_at_one() {
        let tasks = vec![task("A", 2.0, 4.0, 3.0, 2.0), task("B", 6.0, 8.0, 9.0, 5.0)];

        let relative = to_relative_scale(&tasks);
        assert_eq!(relative[0].business_value, 1.0);
        assert_eq!(relative[1].business_value, 3.0);
        assert_eq!(relative[1].time_criticality, 2.0);
        assert_eq!(relative[1].risk_reduction, 3.0);
        assert_eq!(relative[1].effort, 5.0, "effort keeps real units");
    }

    #[test]
    fn relative_scale_snaps_and_caps_on_fibonacci() {
        let tasks = vec![
            task("A", 1.0, 1.0, 1.0, 1.0),
            task("B", 7.0, 11.0, 99.0, 1.0),
        ];

        let relative = to_relative_scale(&tasks);
        assert_eq!(relative[1].business_value, 8.0);
        assert_eq!(relative[1].time_criticality, 13.0);
        assert_eq!(relative[1].risk_reduction, 20.0);
    }

    #[test]
    fn relative_scale_handles_zero_scores() {
        let tasks = vec![task("A", 0.0, 0.0, 0.0, 1.0), task("B", 2.0, 0.0, 4.0, 1.0)];

        let relative = to_relative_scale(&tasks);
        assert_eq!(relative[0].business_value, 1.0);
        assert_eq!(relative[1].business_value, 3.0);
        assert_eq!(relative[1].time_criticality, 1.0);
        assert_eq!(relative[1].risk_reduction, 5.0);
    }

    #[test]
    fn validation_names_the_bad_task_and_field() {
        let err = validate_tasks(&[task("A", 1.0, f64::NAN, 1.0, 1.0)]).unwrap_err();
        assert!(
            err.contains("'A'") && err.contains("time_criticality"),
            "{err}"
        );
        let err = validate_tasks(&[task("B", -1.0, 1.0, 1.0, 1.0)]).unwrap_err();
        assert!(err.contains("business_value"), "{err}");
        assert!(validate_tasks(&[task("A", 1.0, 1.0, 1.0, 1.0)]).is_ok());
    }

    #[test]
    fn validation_rejects_blank_and_duplicate_names() {
        assert!(validate_tasks(&[task("  ", 1.0, 1.0, 1.0, 1.0)]).is_err());
        let err = validate_tasks(&[
            task("Ship", 1.0, 1.0, 1.0, 1.0),
            task("ship", 2.0, 1.0, 1.0, 1.0),
        ])
        .unwrap_err();
        assert!(err.contains("used twice"), "{err}");
    }

    fn after(mut t: Task, deps: &[&str]) -> Task {
        t.depends_on = deps.iter().map(|d| d.to_string()).collect();
        t
    }

    #[test]
    fn a_task_waits_for_its_dependency() {
        // "Launch" scores highest but needs "Schema", which only fits after it.
        let tasks = vec![
            after(task("Launch", 9.0, 9.0, 9.0, 2.0), &["Schema"]),
            task("Schema", 2.0, 2.0, 2.0, 2.0),
            task("Docs", 3.0, 3.0, 3.0, 2.0),
        ];
        let plan = select_batch_detailed(&tasks, 4.0);
        let names: Vec<&str> = plan.selected.iter().map(|r| r.task.name.as_str()).collect();
        assert_eq!(names, vec!["Launch", "Schema"]);
        assert!(plan.blocked.is_empty());
    }

    #[test]
    fn a_task_whose_dependency_does_not_fit_is_reported_blocked() {
        let tasks = vec![
            after(task("Launch", 9.0, 9.0, 9.0, 1.0), &["Migrate"]),
            task("Migrate", 1.0, 1.0, 1.0, 10.0),
        ];
        let plan = select_batch_detailed(&tasks, 3.0);
        assert!(plan.selected.is_empty());
        assert_eq!(plan.blocked, vec!["Launch".to_string()]);
    }

    #[test]
    fn dependencies_outside_the_input_count_as_done() {
        let tasks = vec![after(
            task("Launch", 9.0, 9.0, 9.0, 1.0),
            &["Already shipped"],
        )];
        assert_eq!(select_batch_detailed(&tasks, 3.0).selected.len(), 1);
    }

    #[test]
    fn dependency_loops_are_rejected() {
        let tasks = vec![
            after(task("A", 1.0, 1.0, 1.0, 1.0), &["B"]),
            after(task("B", 1.0, 1.0, 1.0, 1.0), &["C"]),
            after(task("C", 1.0, 1.0, 1.0, 1.0), &["a"]),
        ];
        let err = validate_tasks(&tasks).unwrap_err();
        assert!(err.contains("loop"), "{err}");
    }
}
