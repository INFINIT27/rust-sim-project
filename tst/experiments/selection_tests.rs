//! Unit tests for the [`crate::experiments::selection`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/experiments/selection.rs` via `#[path]`, not as a standalone integration test.
//!
//! The parse is the app's entire command-line surface, and it is the one part a user
//! interacts with before anything renders — so these cover the accepted forms, the
//! error cases *by message-carrying variant* rather than just "is an error", and the
//! two flags that short-circuit.

use super::*;

/// Parse a `&str` argument list, panicking on error — for the cases that must succeed.
fn parse(args: &[&str]) -> Command {
    parse_args(args.iter().copied()).expect("these arguments must parse")
}

/// The selection from a successful run command.
fn selection(args: &[&str]) -> Selection {
    match parse(args) {
        Command::Run(selection) => selection,
        other => panic!("expected a run, got {other:?}"),
    }
}

#[test]
fn no_arguments_runs_the_default_experiment() {
    let empty: [&str; 0] = [];
    assert_eq!(parse(&empty), Command::Run(Selection::default()));
    assert_eq!(selection(&empty).id, ExperimentId::TwoBody);
    assert_eq!(selection(&empty).seed, None, "no seed means a fresh one");
}

#[test]
fn an_experiment_can_be_named_by_slug_number_or_alias() {
    for token in ["experiment_1", "1", "two-body", "two_body"] {
        assert_eq!(selection(&[token]).id, ExperimentId::TwoBody, "{token}");
    }
    for token in ["experiment_2", "2", "three-body", "three_body", "ring"] {
        assert_eq!(
            selection(&[token]).id,
            ExperimentId::ThreeBodyRing,
            "{token}"
        );
    }
}

#[test]
fn naming_is_case_and_whitespace_insensitive() {
    assert_eq!(selection(&["EXPERIMENT_2"]).id, ExperimentId::ThreeBodyRing);
    assert_eq!(
        selection(&["  three-body  "]).id,
        ExperimentId::ThreeBodyRing
    );
}

#[test]
fn empty_arguments_are_skipped_rather_than_rejected() {
    // A shell expanding an unset variable produces these, and failing on one would be
    // an unhelpful surprise.
    assert_eq!(
        selection(&["", "experiment_2", ""]).id,
        ExperimentId::ThreeBodyRing
    );
}

#[test]
fn a_seed_can_be_given_separated_or_joined() {
    assert_eq!(selection(&["--seed", "42"]).seed, Some(42));
    assert_eq!(selection(&["--seed=42"]).seed, Some(42));
    // Underscores group a long literal, as in Rust source.
    assert_eq!(selection(&["--seed", "1_000_000"]).seed, Some(1_000_000));
    // And the full range is usable.
    assert_eq!(
        selection(&["--seed", "18446744073709551615"]).seed,
        Some(u64::MAX)
    );
}

#[test]
fn a_seed_and_an_experiment_combine_in_either_order() {
    let first = selection(&["experiment_2", "--seed", "7"]);
    let second = selection(&["--seed", "7", "experiment_2"]);

    assert_eq!(first, second);
    assert_eq!(first.id, ExperimentId::ThreeBodyRing);
    assert_eq!(first.seed, Some(7));
}

#[test]
fn the_last_seed_wins() {
    // Not an error: a repeated flag overriding is the conventional shell behaviour, and
    // it is what makes appending to a saved command line work.
    assert_eq!(selection(&["--seed", "1", "--seed", "2"]).seed, Some(2));
}

#[test]
fn help_and_list_short_circuit() {
    for token in ["--help", "-h", "help"] {
        assert_eq!(parse(&[token]), Command::Help, "{token}");
    }
    for token in ["--list", "-l", "list"] {
        assert_eq!(parse(&[token]), Command::List, "{token}");
    }

    // Terminal, so they win over anything after them — including nonsense. Someone
    // asking what the options are should be told, not corrected.
    assert_eq!(parse(&["--list", "--nonsense"]), Command::List);
    assert_eq!(parse(&["experiment_2", "--help"]), Command::Help);
}

#[test]
fn an_unknown_experiment_is_reported_with_the_token() {
    let error = parse_args(["experiment_9"]).expect_err("must not parse");
    assert_eq!(
        error,
        ParseError::UnknownExperiment("experiment_9".to_string())
    );
    // The message has to name the token, or the user re-reads their own command line to
    // find out which word was wrong.
    assert!(error.to_string().contains("experiment_9"));
    assert!(error.to_string().contains("--list"));
}

#[test]
fn a_missing_or_invalid_seed_is_reported() {
    assert_eq!(
        parse_args(["--seed"]).expect_err("must not parse"),
        ParseError::MissingSeedValue
    );
    assert_eq!(
        parse_args(["--seed="]).expect_err("must not parse"),
        ParseError::MissingSeedValue
    );

    let error = parse_args(["--seed", "banana"]).expect_err("must not parse");
    assert_eq!(error, ParseError::InvalidSeed("banana".to_string()));
    assert!(error.to_string().contains("banana"));

    // Negative and fractional seeds are not `u64`s. Note `--seed -1` is reported as an
    // invalid *seed* rather than an unknown flag: the token after `--seed` is consumed
    // as its value whatever it looks like, which gives the more useful message of the
    // two.
    assert_eq!(
        parse_args(["--seed", "-1"]).expect_err("must not parse"),
        ParseError::InvalidSeed("-1".to_string())
    );
    assert_eq!(
        parse_args(["--seed", "1.5"]).expect_err("must not parse"),
        ParseError::InvalidSeed("1.5".to_string())
    );
}

#[test]
fn an_unknown_flag_is_reported_rather_than_ignored() {
    let error = parse_args(["--turbo"]).expect_err("must not parse");
    assert_eq!(error, ParseError::UnknownFlag("--turbo".to_string()));
    assert!(error.to_string().contains("--turbo"));
}

#[test]
fn a_second_experiment_is_refused() {
    // Silently picking one would run something the user did not ask for.
    let error = parse_args(["experiment_1", "experiment_2"]).expect_err("must not parse");
    assert_eq!(
        error,
        ParseError::TooManyExperiments("experiment_2".to_string())
    );
}

#[test]
fn an_explicit_seed_resolves_to_itself() {
    let selection = Selection {
        id: ExperimentId::ThreeBodyRing,
        seed: Some(2024),
    };
    assert_eq!(selection.resolve_seed(), 2024);
    // Repeatedly — resolution must not consume or advance anything.
    assert_eq!(selection.resolve_seed(), 2024);
}

#[test]
fn an_absent_seed_resolves_to_something_fresh() {
    // Derived from the wall clock, so two resolutions moments apart normally differ and
    // each launch of a randomised experiment is a new one. Asserting only that it is
    // non-zero and usable: asserting two calls differ would be a flaky test on a coarse
    // clock.
    let selection = Selection {
        id: ExperimentId::ThreeBodyRing,
        seed: None,
    };

    assert_ne!(selection.resolve_seed(), 0);
}

#[test]
fn the_help_text_lists_every_experiment_and_the_window_keys() {
    let text = help_text();

    for id in ExperimentId::ALL {
        assert!(text.contains(id.slug()), "help is missing {}", id.slug());
        assert!(text.contains(id.name()), "help is missing {}", id.name());
    }

    // The controls are only discoverable here, so their absence would be a real gap.
    assert!(text.contains("--seed"));
    assert!(text.contains("Pause"));
    assert!(text.contains("time scale"));
}
