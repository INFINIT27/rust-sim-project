//! Choosing an experiment from the command line.
//!
//! [`parse_args`] turns the process arguments into a [`Command`], and `main.rs` does
//! what it says. Everything here is a pure function over an iterator of strings, so
//! the whole surface is testable without running the binary — which is the point of it
//! not living in `main.rs`.
//!
//! # The grammar
//!
//! ```text
//! physics-sandbox [<experiment>] [--seed <u64>] [--list] [--help]
//! ```
//!
//! - `<experiment>` — a slug (`experiment_1`), a number (`2`), or a descriptive alias
//!   (`three-body`). Absent means [`ExperimentId::default`].
//! - `--seed <u64>` or `--seed=<u64>` — fix the randomised initial conditions so a run
//!   can be reproduced exactly. Ignored by experiments that have nothing to randomise.
//! - `--list` — print the catalog and exit.
//! - `--help` / `-h` — print usage and exit.
//!
//! The in-window key bindings `--help` prints come from
//! [`crate::bindings::binding_help_text`], from the same rows the `Esc`
//! menu draws and the same defaults the input systems start on, so they cannot disagree.
//!
//! # Why hand-rolled rather than `clap`
//!
//! Four flags, no subcommands, no completions. The dependency would be larger than the
//! program's entire argument surface, and the parse is thirty lines with tests. That
//! calculus flips as soon as this grows a third positional or needs `--help` to be
//! generated rather than written — at that point take `clap` rather than growing this.
//!
//! # Why the GUI will not need to change this
//!
//! [`Selection`] is the thing the app actually consumes: an id and a seed. The command
//! line is one way to produce one; a picker in the overlay would be another, and would
//! build a `Selection` directly without touching this module. That is why the parse
//! returns data rather than mutating anything.

use crate::bindings::binding_help_text;
use crate::experiments::catalog::{catalog_lines, ExperimentId};
use crate::experiments::constants::DEFAULT_RANDOM_SEED;

/// What the caller asked for: an experiment, and optionally a seed for it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    /// Which experiment to run.
    pub id: ExperimentId,
    /// The seed to build it with, or `None` to let the run choose one.
    ///
    /// `None` means "a fresh experiment": [`resolve_seed`](Self::resolve_seed) derives
    /// one from the wall clock, so each launch of a randomised experiment differs. An
    /// explicit seed is how a run is repeated.
    pub seed: Option<u64>,
}

impl Default for Selection {
    /// The default experiment, with no fixed seed.
    #[inline]
    fn default() -> Self {
        Self {
            id: ExperimentId::default(),
            seed: None,
        }
    }
}

impl Selection {
    /// The seed to build with: the explicit one, or a fresh one derived from the wall
    /// clock.
    ///
    /// # Why the clock
    ///
    /// A randomised experiment whose randomness is the same every launch is not an
    /// experiment, it is a fixture. Deriving the fallback from
    /// `SystemTime::UNIX_EPOCH`'s elapsed nanoseconds means every run of
    /// `experiment_2` is a different one, and `--seed` is how you go back to a run you
    /// liked. The seed is recorded in the spec and shown in the overlay so "a run you
    /// liked" is always identifiable after the fact.
    ///
    /// Falls back to [`DEFAULT_RANDOM_SEED`] if the clock is before the epoch, which is
    /// not a case worth failing over.
    ///
    /// Note this is the one place in the crate that reads the *real world's* clock
    /// rather than a frame delta — and it is `std::time`, not Bevy, so it does not
    /// breach this layer's no-Bevy rule.
    pub fn resolve_seed(&self) -> u64 {
        if let Some(seed) = self.seed {
            return seed;
        }

        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_nanos() as u64)
            .unwrap_or(DEFAULT_RANDOM_SEED)
    }
}

/// What `main.rs` should do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Open the window and run this selection.
    Run(Selection),
    /// Print the catalog and exit successfully.
    List,
    /// Print usage and exit successfully.
    Help,
}

/// Why a command line could not be understood.
///
/// Carries the offending token so the message can quote it: an error that says only
/// "invalid argument" makes the user re-read their own command line to find out which.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// A positional argument that names no experiment.
    UnknownExperiment(String),
    /// `--seed` with nothing after it.
    MissingSeedValue,
    /// `--seed` with something that is not a `u64`.
    InvalidSeed(String),
    /// A flag that does not exist.
    UnknownFlag(String),
    /// A second positional argument — one experiment per run.
    TooManyExperiments(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownExperiment(token) => {
                write!(f, "unknown experiment `{token}` — try --list")
            }
            Self::MissingSeedValue => write!(f, "--seed needs a value, e.g. --seed 42"),
            Self::InvalidSeed(token) => {
                write!(f, "`{token}` is not a seed — expected a whole number")
            }
            Self::UnknownFlag(flag) => write!(f, "unknown flag `{flag}` — try --help"),
            Self::TooManyExperiments(token) => write!(
                f,
                "only one experiment per run, but `{token}` was given as well"
            ),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse the arguments *after* the program name.
///
/// `main.rs` passes `std::env::args().skip(1)`. Taking an iterator rather than reading
/// the environment is what makes every case below testable.
///
/// # Returns
///
/// The [`Command`] to carry out, or a [`ParseError`] naming the token at fault. An
/// empty argument list is valid and yields `Command::Run(Selection::default())`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{parse_args, Command, ExperimentId};
///
/// // No arguments: the default experiment, no fixed seed.
/// let command = parse_args(std::iter::empty::<String>()).unwrap();
/// assert_eq!(command, Command::Run(Default::default()));
///
/// // A slug and a seed.
/// let args = ["experiment_2", "--seed", "42"].map(String::from);
/// match parse_args(args.into_iter()).unwrap() {
///     Command::Run(selection) => {
///         assert_eq!(selection.id, ExperimentId::ThreeBodyRing);
///         assert_eq!(selection.seed, Some(42));
///     }
///     other => panic!("expected a run, got {other:?}"),
/// }
/// ```
pub fn parse_args<I, S>(args: I) -> Result<Command, ParseError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut selection = Selection::default();
    let mut chose_experiment = false;
    let mut args = args.into_iter().peekable();

    while let Some(argument) = args.next() {
        let argument = argument.as_ref().trim().to_string();

        if argument.is_empty() {
            continue;
        }

        // `--help` and `--list` are terminal: they short-circuit, so `--list
        // --nonsense` prints the list rather than complaining. That is the friendlier
        // reading of an argument list whose author is already asking what the options
        // are.
        match argument.as_str() {
            "--help" | "-h" | "help" => return Ok(Command::Help),
            "--list" | "-l" | "list" => return Ok(Command::List),
            _ => {}
        }

        if let Some(value) = argument.strip_prefix("--seed=") {
            selection.seed = Some(parse_seed(value)?);
            continue;
        }

        if argument == "--seed" {
            let value = args.next().ok_or(ParseError::MissingSeedValue)?;
            selection.seed = Some(parse_seed(value.as_ref())?);
            continue;
        }

        if argument.starts_with('-') {
            return Err(ParseError::UnknownFlag(argument));
        }

        // Positional: the experiment.
        if chose_experiment {
            return Err(ParseError::TooManyExperiments(argument));
        }

        let id = ExperimentId::ALL
            .iter()
            .copied()
            .find(|id| id.matches(&argument))
            .ok_or_else(|| ParseError::UnknownExperiment(argument.clone()))?;

        selection.id = id;
        chose_experiment = true;
    }

    Ok(Command::Run(selection))
}

/// Parse a seed, accepting `_` separators so a long literal can be grouped.
fn parse_seed(token: &str) -> Result<u64, ParseError> {
    let cleaned: String = token
        .chars()
        .filter(|character| *character != '_')
        .collect();

    if cleaned.is_empty() {
        return Err(ParseError::MissingSeedValue);
    }

    cleaned
        .parse::<u64>()
        .map_err(|_| ParseError::InvalidSeed(token.to_string()))
}

/// The usage text `--help` prints.
///
/// Written out rather than generated, and it lists the catalog by calling
/// [`catalog_lines`] so a new experiment appears in `--help` without anyone
/// remembering to add it.
pub fn help_text() -> String {
    let mut text = String::from(
        "physics-sandbox — a small 3D gravitation and contact simulator\n\
         \n\
         USAGE:\n\
         \u{20}   cargo run --release -- [<experiment>] [--seed <number>]\n\
         \n\
         OPTIONS:\n\
         \u{20}   --seed <number>   Fix the randomised initial conditions, so a run can be repeated\n\
         \u{20}   --list, -l        List the experiments and exit\n\
         \u{20}   --help, -h        Print this and exit\n\
         \n\
         EXPERIMENTS:\n",
    );

    for line in catalog_lines() {
        text.push_str("    ");
        text.push_str(&line);
        text.push('\n');
    }

    text.push_str(
        "\nAn experiment may be named by its slug (`experiment_2`), its number (`2`),\n\
         or a descriptive alias (`three-body`). With no experiment given, \
         `experiment_1` runs.\n\
         \n\
         NOTE ON experiment_3:\n\
         \u{20}   Everything is at true scale and nothing compensates for it, so it opens on\n\
         \u{20}   almost nothing: the Sun is 4.6 pixels across from the starting camera and\n\
         \u{20}   the planets are under half a pixel, so they are not drawn at all. The orbit\n\
         \u{20}   trails show where they are. Fly to one (W; C cycles the camera speed, one\n\
         \u{20}   press for 3x) and it becomes an object at exactly the right size. That is\n\
         \u{20}   not a bug: at real\n\
         \u{20}   distances a planet is sub-pixel from anywhere you can see its orbit.\n\
         \u{20}   No single clock scale suits both ends either -- the shipped one gives one\n\
         \u{20}   Earth year every ~32 seconds, which makes one Neptune orbit about 1.7\n\
         \u{20}   hours. Use ] and [. For a system that IS visible at true scale, try\n\
         \u{20}   experiment_4: a moon is enormous compared to its orbit.\n\
         \n\
         IN THE WINDOW:\n",
    );

    // The bindings are not written out here: they come from `crate::bindings`, the same rows
    // the in-window `Esc` menu draws. Two hand-maintained copies of a key list drift the
    // first time a binding changes — in the one place a user looks to find out what the
    // bindings are.
    text.push_str(&binding_help_text());

    text.push_str(
        "\n    The window is full-screen and has no close button; see Window above.\n\
         \u{20}   Press Esc in the window for this list on screen, with the current settings.\n",
    );

    text
}

#[cfg(test)]
#[path = "../../tst/experiments/selection_tests.rs"]
mod tests;
