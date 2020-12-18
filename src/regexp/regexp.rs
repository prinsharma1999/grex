/*
 * Copyright © 2019-2020 Peter M. Stahl pemistahl@gmail.com
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either expressed or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use crate::ast::Expression;
use crate::char::{ColorizableString, GraphemeCluster};
use crate::fsm::DFA;
use crate::regexp::config::RegExpConfig;
use colored::ColoredString;
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use std::cmp::Ordering;
use std::fmt::{Display, Formatter, Result};

pub struct RegExp {
    ast: Expression,
    config: RegExpConfig,
}

impl RegExp {
    pub(crate) fn from(test_cases: &mut Vec<String>, config: &RegExpConfig) -> Self {
        if config.is_case_insensitive_matching() {
            Self::convert_to_lowercase(test_cases);
        }
        Self::sort(test_cases);
        let grapheme_clusters = Self::grapheme_clusters(&test_cases, config);
        let dfa = DFA::from(grapheme_clusters, config);
        let ast = Expression::from(dfa, config);
        Self {
            ast,
            config: config.clone(),
        }
    }

    #[allow(unused_must_use)]
    fn convert_to_lowercase(test_cases: &mut Vec<String>) {
        std::mem::replace(
            test_cases,
            test_cases.iter().map(|it| it.to_lowercase()).collect_vec(),
        );
    }

    fn sort(test_cases: &mut Vec<String>) {
        test_cases.sort();
        test_cases.dedup();
        test_cases.sort_by(|a, b| match a.len().cmp(&b.len()) {
            Ordering::Equal => a.cmp(&b),
            other => other,
        });
    }

    fn grapheme_clusters(test_cases: &[String], config: &RegExpConfig) -> Vec<GraphemeCluster> {
        let mut clusters = test_cases
            .iter()
            .map(|it| GraphemeCluster::from(it, config))
            .collect_vec();

        if config.is_char_class_feature_enabled() {
            for cluster in clusters.iter_mut() {
                cluster.convert_to_char_classes();
            }
        }

        if config.is_repetition_converted() {
            for cluster in clusters.iter_mut() {
                cluster.convert_repetitions();
            }
        }

        clusters
    }
}

impl Display for RegExp {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let (
            ignore_case_flag,
            verbose_mode_flag,
            left_anchor,
            left_parenthesis,
            right_parenthesis,
            right_anchor,
        ) = to_colorized_string(
            vec![
                if self.config.is_case_insensitive_matching() {
                    ColorizableString::IgnoreCaseFlag
                } else {
                    ColorizableString::EmptyString
                },
                ColorizableString::VerboseModeFlag,
                ColorizableString::Caret,
                if self.config.is_capturing_group_enabled() {
                    ColorizableString::CapturingLeftParenthesis
                } else {
                    ColorizableString::NonCapturingLeftParenthesis
                },
                ColorizableString::RightParenthesis,
                ColorizableString::DollarSign,
            ],
            &self.config,
        );

        let mut regexp = match self.ast {
            Expression::Alternation(_, _) => format!(
                "{}{}{}{}{}{}",
                ignore_case_flag,
                left_anchor,
                left_parenthesis,
                self.ast.to_string(),
                right_parenthesis,
                right_anchor
            ),
            _ => format!(
                "{}{}{}{}",
                ignore_case_flag,
                left_anchor,
                self.ast.to_string(),
                right_anchor
            ),
        };

        if regexp.contains("\u{b}") {
            regexp = regexp.replace("\u{b}", "\\v"); // U+000B Line Tabulation
        }

        if self.config.is_verbose_mode_enabled {
            write!(f, "{}", apply_verbose_mode(regexp, verbose_mode_flag))
        } else {
            write!(f, "{}", regexp)
        }
    }
}

fn to_colorized_string(
    strings: Vec<ColorizableString>,
    config: &RegExpConfig,
) -> (
    ColoredString,
    ColoredString,
    ColoredString,
    ColoredString,
    ColoredString,
    ColoredString,
) {
    let v = strings
        .iter()
        .map(|it| it.to_colorized_string(config.is_output_colorized))
        .collect_vec();

    (
        v[0].clone(),
        v[1].clone(),
        v[2].clone(),
        v[3].clone(),
        v[4].clone(),
        v[5].clone(),
    )
}

fn apply_verbose_mode(regexp: String, verbose_mode_flag: ColoredString) -> String {
    lazy_static! {
        static ref VERBOSE_MODE_REGEX: Regex = Regex::new(
            r#"(?x)
            (?:
                \u{1b}\[
                (?:
                    1;31m   # red bold
                    |
                    1;35m   # purple bold
                    |
                    1;33m   # yellow bold
                    |
                    1;32m   # green bold
                    |
                    1;36m   # cyan bold
                    |
                    104;37m # white on bright blue
                    |
                    40;93m  # bright yellow on black
                    |
                    103;30m # black on bright yellow
                )
            )?
            (?:
                \(\?:
                |
                \[.+\]
                |
                \\[\^(){}\[\]|$*+?\\nrtv.-]
                |
                [\^(){}\[\]|$*+?\\.-]
                |
                [^\^(){}\[\]|$*+?\\.-]+
            )
            (?:\u{1b}\[0m)? # color reset
            "#,
        )
        .unwrap();
    }

    let mut verbose_regexp = vec![verbose_mode_flag.to_string()];
    let mut nesting_level = 0;

    for match_part in VERBOSE_MODE_REGEX.find_iter(&regexp) {
        let substr = match_part
            .as_str()
            .to_string()
            .replace("#", "\\#")
            .replace(" ", "\\s")
            .replace(" ", "\\s")
            .replace(" ", "\\s")
            .replace(" ", "\\s")
            .replace(" ", "\\s")
            .replace(" ", "\\s")
            .replace(" ", "\\s")
            .replace("\u{85}", "\\s")
            .replace(" ", "\\ ");

        let is_char_class = substr.starts_with("[") && substr.ends_with("]");

        if !is_char_class && substr.contains(')') && !substr.contains("\\)") {
            nesting_level -= 1;
        }

        let indentation = "  ".repeat(nesting_level);
        verbose_regexp.push(format!("{}{}", indentation, substr));

        if substr.contains('(') && !substr.contains("\\(") {
            nesting_level += 1;
        }
    }

    verbose_regexp.join("\n")
}
