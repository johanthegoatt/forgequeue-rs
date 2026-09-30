use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub name: String,
    pub business_value: f64,
    pub time_criticality: f64,
    pub risk_reduction: f64,
    pub effort: f64,
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

fn select_from_ranked(ranked: Vec<RankedTask>, budget: f64) -> BatchPlan {
    let mut selected = Vec::new();
    let mut used = 0.0;
    let mut skipped = 0;
    let safe_budget = budget.max(0.0);

    for ranked_task in ranked {
        let effort = normalized_effort(ranked_task.task.effort);
        if used + effort > safe_budget {
            skipped += 1;
            continue;
        }
        used += effort;
        selected.push(ranked_task);
    }

    BatchPlan {
        budget: safe_budget,
        used_effort: used,
        skipped_tasks: skipped,
        selected,
    }
}

pub fn select_batch_detailed(tasks: &[Task], budget: f64) -> BatchPlan {
    let ranked = prioritize(tasks);
    select_from_ranked(ranked, budget)
}

pub fn select_batch_detailed_filtered(
    tasks: &[Task],
    budget: f64,
    min_score: f64,
    top: usize,
) -> BatchPlan {
    let ranked = prioritize_with_filters(tasks, min_score, top);
    select_from_ranked(ranked, budget)
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
    use super::{prioritize, prioritize_with_filters, select_batch, to_relative_scale, wsjf, Task};

    #[test]
    fn prioritize_orders_by_highest_score_first() {
        let tasks = vec![
            Task {
                name: "Low".to_string(),
                business_value: 3.0,
                time_criticality: 2.0,
                risk_reduction: 2.0,
                effort: 4.0,
            },
            Task {
                name: "High".to_string(),
                business_value: 9.0,
                time_criticality: 9.0,
                risk_reduction: 7.0,
                effort: 3.0,
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
            },
            Task {
                name: "Alpha".to_string(),
                business_value: 5.0,
                time_criticality: 5.0,
                risk_reduction: 5.0,
                effort: 3.0,
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
            },
            Task {
                name: "B".to_string(),
                business_value: 3.0,
                time_criticality: 3.0,
                risk_reduction: 3.0,
                effort: 2.0,
            },
            Task {
                name: "C".to_string(),
                business_value: 7.0,
                time_criticality: 7.0,
                risk_reduction: 6.0,
                effort: 2.0,
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
            },
            Task {
                name: "B".to_string(),
                business_value: 8.0,
                time_criticality: 8.0,
                risk_reduction: 8.0,
                effort: 5.0,
            },
            Task {
                name: "C".to_string(),
                business_value: 6.0,
                time_criticality: 6.0,
                risk_reduction: 6.0,
                effort: 3.0,
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
}
