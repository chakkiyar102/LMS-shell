use crate::content::Question;
use shared::QuestionResult;

/// `picked`: option indices for single/multi, chosen right-index per left row for match.
pub fn grade(q: &Question, picked: &[usize]) -> QuestionResult {
    match q {
        Question::Single { options, answer, .. } => QuestionResult {
            score: (picked == [*answer]) as u8 as f32,
            feedback: picked.iter().filter_map(|i| options.get(*i)).map(|o| o.feedback.clone()).collect(),
        },
        Question::Multi { options, answer, .. } => {
            // wrong flags cost marks, floor at zero
            let hits = picked.iter().filter(|p| answer.contains(p)).count() as f32;
            let misses = picked.len() as f32 - hits;
            QuestionResult {
                score: ((hits - misses) / answer.len().max(1) as f32).max(0.0),
                feedback: picked.iter().filter_map(|i| options.get(*i)).map(|o| o.feedback.clone()).collect(),
            }
        }
        Question::Match { answer, feedback, .. } => QuestionResult {
            score: answer.iter().zip(picked).filter(|(a, p)| a == p).count() as f32 / answer.len().max(1) as f32,
            feedback: feedback.clone(),
        },
    }
}

pub fn grade_all(qs: &[&Question], answers: &[Vec<usize>], pass_mark: f32) -> Result<shared::Graded, String> {
    if qs.len() != answers.len() {
        return Err(format!("expected {} answers, got {}", qs.len(), answers.len()));
    }
    let results: Vec<_> = qs.iter().zip(answers).map(|(q, a)| grade(q, a)).collect();
    let score = results.iter().map(|r| r.score).sum::<f32>() / results.len().max(1) as f32;
    Ok(shared::Graded { score, passed: score >= pass_mark, results })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Opt;

    fn opts(n: usize) -> Vec<Opt> {
        (0..n).map(|i| Opt { text: format!("o{i}"), feedback: format!("f{i}") }).collect()
    }

    #[test]
    fn grading() {
        let m = Question::Multi { prompt: "".into(), options: opts(4), answer: vec![0, 2] };
        assert_eq!(grade(&m, &[0, 2]).score, 1.0);
        assert_eq!(grade(&m, &[0, 1]).score, 0.0); // one hit, one wrong flag
        assert_eq!(grade(&m, &[0]).score, 0.5);
        assert_eq!(grade(&m, &[0, 1]).feedback, vec!["f0", "f1"]);

        let s = Question::Single { prompt: "".into(), options: opts(3), answer: 1 };
        assert_eq!(grade(&s, &[1]).score, 1.0);
        assert_eq!(grade(&s, &[1, 0]).score, 0.0);
        assert_eq!(grade(&s, &[7]).feedback.len(), 0); // out-of-range pick: no panic

        let x = Question::Match { prompt: "".into(), left: vec![], right: vec![], answer: vec![0, 5, 2, 0], feedback: vec![] };
        assert_eq!(grade(&x, &[0, 5, 0, 0]).score, 0.75);
        assert_eq!(grade(&x, &[0]).score, 0.25); // short answer counts missing rows wrong

        let g = grade_all(&[&s, &m], &[vec![1], vec![0]], 0.75).unwrap();
        assert_eq!((g.score, g.passed), (0.75, true));
        assert!(grade_all(&[&s], &[], 0.5).is_err());
    }
}
