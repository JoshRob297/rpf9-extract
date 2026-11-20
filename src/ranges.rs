use std::ops::Range;

pub fn gaps<'a>(
    within: &Range<usize>,
    sorted: impl IntoIterator<Item = &'a Range<usize>>,
) -> Vec<Range<usize>> {
    let (mut gaps, mut at) = (Vec::new(), within.start);
    for taken in sorted {
        let until = taken.start.min(within.end);
        if at < until {
            gaps.push(at..until);
        }
        at = at.max(taken.end);
    }
    if at < within.end {
        gaps.push(at..within.end);
    }
    gaps
}
