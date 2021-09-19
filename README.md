#saw
saw is a command-line application written in rust that aims to combine common use cases of **s**ed and **aw**k into one tool with an intuitive and easy-to-use syntax.

saw is still in development and is not feature-complete yet.

#Usage
saw processes text line-by-line either from a file or from standard input. 
Its basic usage is
```bash
saw [-f <path>] <prog> ...
```
where `<path>` can be the path of an input file.

A saw program consists of a list of "atoms". An atom can be
considered a text processing unit that takes an input string, modifies
it, and then either passes it on to the next atom in the list or prints it
to the screen.

Atoms may also have an internal state that allows them to be "context
aware"; that is, an atom's behaviour might depend on the lines
previously processed.

#Example
Consider the following INI file.
```INI
; example.ini
[Section 1]
name=value1

[Section 2]
name=value2
```
To use saw to extract the line starting with `name` from `[Section 2]`, use
```bash
saw -f example.ini filter-range '^\[Section 2' '^\[' filter '^name'
```
This saw program can be read as:
> First, filter for everything in between lines starting with `[Section
> 2` and `[` ; then filter the remaining lines for ones starting with
> `name`.

 Note that `filter-range` and `filter` can be abbreviated to `fr` and `f`,
 respectively. Other atom identifier have similar abbreviations; details can be found in the Section [List of Atoms](#list-of-atoms) below.

 This is also one of the examples that led to the creation of saw
 since doing this with awk or sed requires a fair amount of
 [boilerplate
 code](https://stackoverflow.com/questions/22550265/read-certain-key-from-certain-section-of-ini-file-sed-awk).

#List of Atoms
saw's atoms always consist of one keyword (that might have one or more
aliases) as well as a fixed number of arguments. The keyword and
arguments are passed as seperate arguments to the `saw` command.

Note that the regular expressions used in `saw` are the ones from rust's
[regex](https://crates.io/crates/regex) crate; it syntax can be found
[here](https://docs.rs/regex/1.5.4/regex/index.html#syntax).

**`filter <regex>`**  
Aliases: **`f`**  
grep-like filter: If a the input matches `<regex>`, it is passed on
without modification. If not, `filter` stops the processing of the
current line and the next line is processed.

**`match <regex>`**  
Aliases: **`m`**  
Restrict the usage of following atoms to lines matching `<regex>`.
It is similar to `filter` with the only difference being that input 
not matching `<regex>` is printed out without modifications instead
of being discarded.

**`sub <regex> <replacement>`**  
Aliases: **`s`**  
Replaces the first match of `<regex>` with `<replacement>`.
Numbered and named capture groups are supported and can be referenced in
`<replacement>` as `${#}` or `${name}`. The braces can be dropped
if the following character is not a character allowed in the name of a
caputure group (See also
[here](https://docs.rs/regex/1.5.4/regex/index.html#grouping-and-flags)).

**`gsub <regex> <replacement>`**  
Aliases: **`g`**  
Like `sub` but replaces all occurences instead of only the first one.

**`enumerate`**  
Aliases: **`enum`**, **`e`**, **`#`**  
Enumerates the input by prepending '`<nr>` ', where `<nr>` is the number
of inputs processed so far. Note that this is not neccessarily the line
number if `enumerate` is behind a `filter` or `match`.

**`fields <fields>`**  
Aliases: **`F`**  
Splits the input at whitespaces and passes on only the fields specified by
`<fields>` seperated by single spaces.
The individual fields are identified by position either by a positive or
by a parenthesized negative integer; `1` and `(-1)` represent the first
and last field, respectively.
`<fields>` is then a comma-separated list of either single fields or
two hyphen-separated fields representing a range of fields with the
upper bound included. The upper and/or lower bounds may be dropped to
represent unbounded ranges. Examples of valid inputs for `<fields>` are
`2`, `1,3-(-2)` and `2,4-`.

**`lines <lines>`**  
Aliases: **`line`**, **`l`**  
Filters the input by count for inputs specified by `<lines>`. The
syntax for `<lines>` is the same as for `<fields>` above with the
difference that only positive integers are allowed.
For example, `lines 1,5-` will only "leave through"
the first input it received and every input starting with the fifth.
If `lines <lines>` is the first atom in a saw program, it simply filters
out all the lines of the input text except for the ones specified by
`<lines>` (hence the name).

**`filter-range <regex1> <regex2>`**  
Aliases: **`fr`**  
Context aware atom that filters the input for blocks beginning with an
input matching `<regex1>` and ending with an input matching `<regex2>`.
It also resets for each block the internal state of all atoms following
it.  
TODO: Add example

**`match-range <regex1> <regex2>`**  
Aliases: **`mr`**  
Restrict the usage of following atoms to blocks of inputs delimited by inputs matching `<regex1>` and `<regex2>`, respectively.
It is similar to `filter-range` with the only difference being that input 
outside of those blocks is printed out without modifications instead
of being discarded.
