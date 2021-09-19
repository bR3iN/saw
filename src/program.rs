use enum_dispatch::enum_dispatch;
use regex::Regex;

#[derive(Debug, PartialEq)]
enum Output {
    Resume(String),
    ResetAndResume(String),
    Return(Option<String>),
}

impl Output {
    fn as_opt(self) -> Option<String> {
        match self {
            Output::Resume(s) => Some(s),
            Output::ResetAndResume(s) => Some(s),
            Output::Return(opt) => opt,
        }
    }
}

#[enum_dispatch]
#[derive(Debug)]
pub enum Atom {
    Enumeration,
    Filter,
    Lines,
    FilterRange,
    Fields,
    Gsub,
    Match,
    MatchRange,
    Sub,
}

#[derive(Debug)]
pub struct Program(Vec<Atom>);

impl Program {
    pub fn new(inner: Vec<Atom>) -> Program {
        Program(inner)
    }

    pub fn run(&mut self, arg: String) -> Option<String> {
        let mut reset = false;

        self.0
            .iter_mut()
            .fold(Output::Resume(arg), |out, atom| {
                if reset {
                    atom.reset();
                };
                match out {
                    Output::Resume(s) => atom.run(s),
                    Output::ResetAndResume(s) => {
                        reset = true;
                        atom.reset();
                        atom.run(s)
                    }
                    res @ Output::Return(_) => res,
                }
            })
            .as_opt()
    }
}

#[enum_dispatch(Atom)]
trait ProgramAtom {
    fn run(&mut self, arg: String) -> Output;
    fn reset(&mut self) {}
}

#[derive(Debug)]
pub struct Match {
    regex: Regex,
}
impl Match {
    pub fn new(regex: Regex) -> Match {
        Match { regex }
    }
}

impl ProgramAtom for Match {
    fn run(&mut self, arg: String) -> Output {
        if self.regex.is_match(&arg) {
            Output::Resume(arg)
        } else {
            Output::Return(Some(arg))
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FieldId {
    Int(usize),
    FromLast(usize), // FromLast(-1) represents the last field
}
impl FieldId {
    fn to_usize(self, last: usize) -> usize {
        match self {
            FieldId::Int(i) => i,
            FieldId::FromLast(i) => {
                if last + 1 >= i {
                    last + 1 - i
                } else {
                    0
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum FieldsAtom {
    Single(FieldId),
    Range(OpenRange<FieldId>),
}
impl FieldsAtom {
    fn contains(&self, n: usize, last: usize) -> bool {
        match self {
            FieldsAtom::Single(id) => n == id.to_usize(last),
            FieldsAtom::Range(range) => range.map(|id| id.to_usize(last)).contains(n),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Fields(Vec<FieldsAtom>);
impl Fields {
    pub fn contains(&self, n: usize, last: usize) -> bool {
        self.0.iter().any(|fields| fields.contains(n, last))
    }

    pub fn new(source: Vec<FieldsAtom>) -> Fields {
        Fields(source)
    }
}

impl ProgramAtom for Fields {
    fn run(&mut self, arg: String) -> Output {
        let fields: Vec<_> = arg.split(' ').filter(|s| s != &"").collect();
        let len = fields.len();
        Output::Resume(
            fields
                .into_iter()
                .enumerate()
                .filter(|(n, _)| self.contains(n + 1, len))
                .map(|(_, f)| f)
                .intersperse(" ")
                .collect(),
        )
    }
}

#[derive(Debug, PartialEq)]
// possibly unbounded ranges; upper bound is included
pub struct OpenRange<T> {
    lower_bound: Option<T>,
    upper_bound: Option<T>,
}

impl<T> OpenRange<T> {
    pub fn new(lower_bound: Option<T>, upper_bound: Option<T>) -> Self {
        OpenRange {
            lower_bound,
            upper_bound,
        }
    }
    pub fn map<S>(&self, mut f: impl FnMut(&T) -> S) -> OpenRange<S> {
        OpenRange {
            lower_bound: self.lower_bound.as_ref().map(&mut f),
            upper_bound: self.upper_bound.as_ref().map(&mut f),
        }
    }
}

impl<T> OpenRange<T>
where
    T: PartialOrd + PartialEq,
{
    pub fn contains(&self, item: T) -> bool {
        (match self.lower_bound {
            Some(ref bound) => *bound <= item,
            None => true,
        }) && match self.upper_bound {
            Some(ref bound) => item <= *bound,
            None => true,
        }
    }
}

#[derive(Debug)]
pub enum LinesAtom {
    Single(usize),
    Range(OpenRange<usize>),
}
impl LinesAtom {
    pub fn contains(&self, l: usize) -> bool {
        use LinesAtom::*;
        match self {
            Single(n) => l == *n,
            Range(range) => range.contains(l),
        }
    }
}

#[derive(Debug)]
pub struct Lines {
    lines: Vec<LinesAtom>,
    current_line: usize,
}
impl Lines {
    pub fn new(lines: Vec<LinesAtom>) -> Self {
        Lines {
            lines,
            current_line: 0,
        }
    }
}

impl ProgramAtom for Lines {
    fn reset(&mut self) {
        self.current_line = 0
    }

    fn run(&mut self, arg: String) -> Output {
        self.current_line += 1;
        if self
            .lines
            .iter()
            .any(|atom| atom.contains(self.current_line))
        {
            Output::Resume(arg)
        } else {
            Output::Return(None)
        }
    }
}

#[derive(Debug)]
pub struct Filter {
    regex: Regex,
}
impl Filter {
    pub fn new(regex: Regex) -> Filter {
        Filter { regex }
    }
}

impl ProgramAtom for Filter {
    fn run(&mut self, arg: String) -> Output {
        if self.regex.is_match(&arg) {
            Output::Resume(arg)
        } else {
            Output::Return(None)
        }
    }
}

#[derive(Debug)]
enum BlockState {
    Inside,
    Outside,
}

#[derive(Debug)]
pub struct FilterRange {
    start: Regex,
    end: Regex,
    state: BlockState,
}
impl FilterRange {
    pub fn new(start: Regex, end: Regex) -> FilterRange {
        FilterRange {
            start,
            end,
            state: BlockState::Outside,
        }
    }
}

impl ProgramAtom for FilterRange {
    fn run(&mut self, arg: String) -> Output {
        match self.state {
            BlockState::Outside => {
                if self.start.is_match(&arg) {
                    self.state = BlockState::Inside;
                    Output::ResetAndResume(arg)
                } else {
                    Output::Return(None)
                }
            }
            BlockState::Inside => {
                if self.end.is_match(&arg) {
                    self.state = BlockState::Outside;
                };
                Output::Resume(arg)
            }
        }
    }

    fn reset(&mut self) {
        self.state = BlockState::Outside;
    }
}

#[derive(Debug)]
pub struct MatchRange {
    start: Regex,
    end: Regex,
    state: BlockState,
}
impl MatchRange {
    pub fn new(start: Regex, end: Regex) -> MatchRange {
        MatchRange {
            start,
            end,
            state: BlockState::Outside,
        }
    }
}

impl ProgramAtom for MatchRange {
    fn run(&mut self, arg: String) -> Output {
        match self.state {
            BlockState::Outside => {
                if self.start.is_match(&arg) {
                    self.state = BlockState::Inside;
                    Output::ResetAndResume(arg)
                } else {
                    Output::Return(Some(arg))
                }
            }
            BlockState::Inside => {
                if self.end.is_match(&arg) {
                    self.state = BlockState::Outside;
                };
                Output::Resume(arg)
            }
        }
    }

    fn reset(&mut self) {
        self.state = BlockState::Outside;
    }
}

#[derive(Debug)]
pub struct Enumeration {
    current_line: u32,
}
impl Enumeration {
    pub fn new() -> Enumeration {
        Enumeration { current_line: 0 }
    }
}

impl ProgramAtom for Enumeration {
    fn run(&mut self, arg: String) -> Output {
        self.current_line += 1;
        Output::Resume(format!("{} {}", self.current_line.to_string(), arg))
    }

    fn reset(&mut self) {
        self.current_line = 0;
    }
}

// fn compose<S: Program, T: Program>(first: S, second: T) -> impl Program {
//     Composition { first, second }
// }

#[derive(Debug)]
pub struct Sub {
    regex: Regex,
    replacement: String,
}
impl Sub {
    pub fn new(regex: Regex, replacement: String) -> Sub {
        Sub { regex, replacement }
    }
}

impl ProgramAtom for Sub {
    fn run(&mut self, arg: String) -> Output {
        let res = self.regex.replace(&arg, &self.replacement);
        Output::Resume(res.into_owned())
    }
}

#[derive(Debug)]
pub struct Gsub {
    regex: Regex,
    replacement: String,
}
impl Gsub {
    pub fn new(regex: Regex, replacement: String) -> Gsub {
        Gsub { regex, replacement }
    }
}

impl ProgramAtom for Gsub {
    fn run(&mut self, arg: String) -> Output {
        let res = self.regex.replace_all(&arg, &self.replacement);
        Output::Resume(res.into_owned())
    }
}

// pub fn make_filter_sub(regex: Regex, replacement: String) -> impl Program {
//     compose(make_filter(regex.clone()), make_sub(regex, replacement))
// }

// pub fn make_filter_gsub(regex: Regex, replacement: String) -> impl Program {
//     compose(make_filter(regex.clone()), make_gsub(regex, replacement))
// }

#[cfg(test)]
mod tests {
    use super::*;

    static INI_FILE: &'static str = "

    [Header 1]
    key1 = header1_value1
    key2 = header1_value2

    [Header 2]
    key1 = header2_value1
    key2 = header2_value2\
    ";

    static LINE1: &'static str = "";
    static LINE2: &'static str = "[Header 1]";
    static LINE3: &'static str = "key1 = header1_value1";
    static LINE4: &'static str = "key2 = header1_value2";
    static LINE5: &'static str = "";
    static LINE6: &'static str = "[Header 2]";
    static LINE7: &'static str = "key1 = header2_value1";
    static LINE8: &'static str = "key2 = header2_value2";

    #[test]
    fn test_make_enumerate() {
        use Output::*;

        let mut pr = Enumeration::new();

        assert_eq!(pr.run(LINE1.to_owned()), Resume(String::from("1 ") + LINE1));
        assert_eq!(pr.run(LINE2.to_owned()), Resume(String::from("2 ") + LINE2));
        assert_eq!(pr.run(LINE3.to_owned()), Resume(String::from("3 ") + LINE3));
        assert_eq!(pr.run(LINE4.to_owned()), Resume(String::from("4 ") + LINE4));

        pr.reset();

        assert_eq!(pr.run(LINE5.to_owned()), Resume(String::from("1 ") + LINE5));
        assert_eq!(pr.run(LINE6.to_owned()), Resume(String::from("2 ") + LINE6));
        assert_eq!(pr.run(LINE7.to_owned()), Resume(String::from("3 ") + LINE7));
        assert_eq!(pr.run(LINE8.to_owned()), Resume(String::from("4 ") + LINE8));
    }

    #[test]
    fn test_make_filter_from() {
        use Output::*;

        let mut pr = FilterRange::new(
            Regex::new(r"^\[Header 1").unwrap(),
            Regex::new(r"^\[").unwrap(),
        );

        assert_eq!(pr.run(LINE1.to_owned()), Return(None));
        assert_eq!(pr.run(LINE2.to_owned()), Resume(LINE2.to_owned()));
        assert_eq!(pr.run(LINE3.to_owned()), Resume(LINE3.to_owned()));
        assert_eq!(pr.run(LINE4.to_owned()), Resume(LINE4.to_owned()));
        assert_eq!(pr.run(LINE5.to_owned()), Resume(LINE5.to_owned()));
        assert_eq!(pr.run(LINE6.to_owned()), Resume(LINE6.to_owned()));
        assert_eq!(pr.run(LINE7.to_owned()), Return(None));
        assert_eq!(pr.run(LINE8.to_owned()), Return(None));
    }

    #[test]
    fn test_make_match_from() {
        use Output::*;

        let mut pr = MatchRange::new(
            Regex::new(r"^\[Header 1").unwrap(),
            Regex::new(r"^\[").unwrap(),
        );

        assert_eq!(pr.run(LINE1.to_owned()), Return(Some(LINE1.to_owned())));
        assert_eq!(pr.run(LINE2.to_owned()), Resume(LINE2.to_owned()));
        assert_eq!(pr.run(LINE3.to_owned()), Resume(LINE3.to_owned()));
        assert_eq!(pr.run(LINE4.to_owned()), Resume(LINE4.to_owned()));
        assert_eq!(pr.run(LINE5.to_owned()), Resume(LINE5.to_owned()));
        assert_eq!(pr.run(LINE6.to_owned()), Resume(LINE6.to_owned()));
        assert_eq!(pr.run(LINE7.to_owned()), Return(Some(LINE7.to_owned())));
        assert_eq!(pr.run(LINE8.to_owned()), Return(Some(LINE8.to_owned())));
    }

    #[test]
    fn test_make_gsub() {
        use Output::*;

        // Example from the 'regex' docs
        let re = Regex::new(r"(?P<y>\d{4})-(?P<m>\d{2})-(?P<d>\d{2})").unwrap();
        let s = "2012-03-14 and 2014-07-05".to_owned();
        let mut pr = Gsub::new(re, "$m/$d/$y".to_owned());
        assert_eq!(pr.run(s), Resume("03/14/2012 and 07/05/2014".to_owned()));
    }

    #[test]
    fn test_make_sub() {
        use Output::*;

        // Example from the 'regex' docs
        let re = Regex::new("[^01]+").unwrap();
        let mut pr = Sub::new(re, "".to_owned());
        assert_eq!(pr.run("1078910a".to_owned()), Resume("1010a".to_owned()));

        let re = Regex::new("^abc").unwrap();
        let mut pr = Sub::new(re, "".to_owned());
        assert_eq!(pr.run("def".to_owned()), Resume("def".to_owned()));
    }

    #[test]
    fn test_make_match() {
        let mut pr = Match::new(Regex::new("^x: .*").unwrap());

        let string1 = "x: test1".to_owned();
        let string2 = "yx: test2".to_owned();
        assert_eq!(pr.run(string1.clone()), Output::Resume(string1));
        assert_eq!(pr.run(string2.clone()), Output::Return(Some(string2)));
    }

    #[test]
    fn test_make_filter() {
        let mut pr = Filter::new(Regex::new("^x: .*").unwrap());

        let string1 = "x: test1".to_owned();
        let string2 = "yx: test2".to_owned();

        assert_eq!(pr.run(string1.clone()), Output::Resume(string1));
        assert_eq!(pr.run(string2.clone()), Output::Return(None));
    }

    #[test]
    fn test_fields() {
        let mut pr = Fields::new(vec![
            FieldsAtom::Range(FieldId::Int(3), FieldId::FromLast(2)),
            FieldsAtom::Single(FieldId::Int(1)),
        ]);

        let string1 = "1 2 3 4 5 6 7".to_owned();
        let string2 = "1 3 4 5 6".to_owned();
        assert_eq!(pr.run(string1), Output::Resume(string2));
    }
}
