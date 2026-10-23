//! The defect rules of the analyzers of the other languages (Python, Java,
//! Kotlin, Go, Swift, C#, JavaScript and TypeScript): each with its defect
//! class and what it finds. As for C and C++ ([`super::defects`]), a rule
//! the precision study found below the budget (sample precision below 0.80,
//! for a rule with five findings or more on its language's precision
//! corpus) stays listed, `off`, with the reason, and its findings are
//! dropped. A rule with one to four findings keeps its earlier default (off
//! for a rule new to the list), and one with none is on until it is
//! measured. `docs/collect.md` lists them.
//!
//! Standards: SCF TDA-17.1 (Static Code Analysis: each defect rule with its class),
//! VPM-12.2 (Breadth / Depth of Coverage For Vulnerability Scanning).

use crate::collect_convert::{Defect, Rule};

/// The Ruff `--extend-select` argument: every rule of [`RUFF_RULES`]
/// (the catalogue tests keep the two equal) and N999 and S501, which the
/// crown-jewel verdicts retire native rules to.
pub(super) const RUFF_SELECT: &str = "B006,B008,B012,B015,B018,B020,B023,B033,N999,PLR0124,RUF006,RUF012,S110,S307,S501,S602,S605,S608,SIM115";

/// Ruff 0.17.1: the defect rules `collect` selects beyond Ruff's defaults
/// (`--extend-select`, [`RUFF_SELECT`]).
pub(super) const RUFF_RULES: &[Rule] = &[
    Rule {
        id: "B006",
        class: Defect::LanguageTrap,
        reason: "a mutable default argument, shared by every call",
        off: None,
    },
    Rule {
        id: "B008",
        class: Defect::LanguageTrap,
        reason: "a function call in a default argument, evaluated once at definition",
        off: Some(
            "precision 0/20 on the Python precision corpus (105 findings): FastAPI's dependency markers (Depends, Body) in defaults, evaluated once by design",
        ),
    },
    Rule {
        id: "B012",
        class: Defect::LanguageTrap,
        reason: "return, break or continue in finally: it swallows the exception in flight",
        off: None,
    },
    Rule {
        id: "B015",
        class: Defect::WrongOperator,
        reason: "a comparison whose result is thrown away (an assert or assignment was meant)",
        off: None,
    },
    Rule {
        id: "B018",
        class: Defect::WrongOperator,
        reason: "an expression statement with no effect",
        off: None,
    },
    Rule {
        id: "B020",
        class: Defect::WrongOperator,
        reason: "a loop variable that overwrites the iterable it loops over",
        off: None,
    },
    Rule {
        id: "B023",
        class: Defect::LoopCapture,
        reason: "a function defined in a loop that reads the loop variable when called, not when defined",
        off: None,
    },
    Rule {
        id: "B033",
        class: Defect::WrongOperator,
        reason: "a set literal with the same item twice",
        off: None,
    },
    Rule {
        id: "PLR0124",
        class: Defect::WrongOperator,
        reason: "a value compared with itself",
        off: None,
    },
    Rule {
        id: "RUF006",
        class: Defect::UnobservedAsync,
        reason: "an asyncio task whose reference is dropped: it can be garbage-collected before it finishes, and its exception is never seen",
        off: None,
    },
    Rule {
        id: "RUF012",
        class: Defect::LanguageTrap,
        reason: "a mutable class attribute default, shared by every instance",
        off: Some(
            "4 findings on the Python precision corpus, too few to sample (0/4: pydantic fields and read-only class configuration); new to the list, so it stays off",
        ),
    },
    Rule {
        id: "S110",
        class: Defect::SwallowedError,
        reason: "try/except/pass: the error is dropped (kept on: the swallowed-errors transformation reads it)",
        off: None,
    },
    Rule {
        id: "S307",
        class: Defect::Injection,
        reason: "eval of a value that may come from outside",
        off: None,
    },
    Rule {
        id: "S602",
        class: Defect::Injection,
        reason: "subprocess with shell=True: the command line goes through the shell",
        off: None,
    },
    Rule {
        id: "S605",
        class: Defect::Injection,
        reason: "a process started through the shell (os.system, os.popen)",
        off: Some(
            "precision 1/5 on the Python precision corpus (5 findings): constant command lines that nothing from outside reaches",
        ),
    },
    Rule {
        id: "S608",
        class: Defect::Injection,
        reason: "SQL built by string formatting or concatenation",
        off: None,
    },
    Rule {
        id: "SIM115",
        class: Defect::ResourceLeak,
        reason: "a file opened without a context manager: it stays open when an exception is raised",
        off: None,
    },
];

/// The PMD `-R` argument: every rule of [`PMD_RULES`] (the catalogue tests
/// keep the two equal).
pub(super) const PMD_RULESETS: &str = "category/java/errorprone.xml/MisplacedNullCheck,category/java/errorprone.xml/BrokenNullCheck,category/java/errorprone.xml/UnusedNullCheckInEquals,category/java/errorprone.xml/CloseResource,category/java/errorprone.xml/CheckSkipResult,category/java/errorprone.xml/AvoidLosingExceptionInformation,category/java/errorprone.xml/EmptyCatchBlock,category/java/errorprone.xml/ReturnFromFinallyBlock,category/java/errorprone.xml/DoNotThrowExceptionInFinally,category/java/errorprone.xml/CompareObjectsWithEquals,category/java/errorprone.xml/EqualsNull,category/java/errorprone.xml/ComparisonWithNaN,category/java/errorprone.xml/IdempotentOperations,category/java/errorprone.xml/JumbledIncrementer,category/java/errorprone.xml/ImplicitSwitchFallThrough,category/java/errorprone.xml/UnconditionalIfStatement,category/java/errorprone.xml/AvoidBranchingStatementAsLastInLoop,category/java/errorprone.xml/UselessOperationOnImmutable,category/java/errorprone.xml/AssignmentInOperand,category/java/errorprone.xml/SuspiciousEqualsMethodName,category/java/errorprone.xml/OverrideBothEqualsAndHashcode,category/java/errorprone.xml/InvalidLogMessageFormat,category/java/errorprone.xml/AvoidDecimalLiteralsInBigDecimalConstructor,category/java/errorprone.xml/ConstructorCallsOverridableMethod,category/java/multithreading.xml/DoubleCheckedLocking,category/java/multithreading.xml/NonThreadSafeSingleton,category/java/multithreading.xml/UnsynchronizedStaticFormatter";

/// PMD 7.28.0: the Java rules `collect` runs (`-R`, [`PMD_RULESETS`]),
/// from the errorprone and multithreading categories.
pub(super) const PMD_RULES: &[Rule] = &[
    Rule {
        id: "AssignmentInOperand",
        class: Defect::WrongOperator,
        reason: "an assignment inside a condition or other operand",
        off: Some(
            "precision 0/20 on the Java precision corpus (104 findings): read loops, offset += n and x++ in arguments, written on purpose",
        ),
    },
    Rule {
        id: "AvoidBranchingStatementAsLastInLoop",
        class: Defect::WrongOperator,
        reason: "a loop whose last statement always leaves it: it runs at most once",
        off: Some(
            "one finding on the Java precision corpus, too few to sample (0/1: a search loop that returns on its first match); new to the list, so it stays off",
        ),
    },
    Rule {
        id: "AvoidDecimalLiteralsInBigDecimalConstructor",
        class: Defect::Overflow,
        reason: "new BigDecimal(0.1): the binary double's error is kept",
        off: Some(
            "one finding on the Java precision corpus, too few to sample (0/1: 5.5, exact in binary); new to the list, so it stays off",
        ),
    },
    Rule {
        id: "AvoidLosingExceptionInformation",
        class: Defect::SwallowedError,
        reason: "an exception's getMessage() or similar called and the result thrown away",
        off: None,
    },
    Rule {
        id: "BrokenNullCheck",
        class: Defect::NullDereference,
        reason: "a null check that dereferences the value when it is null (|| for &&, or the reverse)",
        off: None,
    },
    Rule {
        id: "CheckSkipResult",
        class: Defect::UncheckedResult,
        reason: "the count InputStream.skip returns ignored: fewer bytes may have been skipped",
        off: None,
    },
    Rule {
        id: "CloseResource",
        class: Defect::ResourceLeak,
        reason: "a stream, connection or other Closeable not closed on every path",
        off: Some(
            "precision 1/20 on the Java precision corpus (434 findings): readers over in-memory strings, wrapped caller streams and the saved System.out, which closing would break",
        ),
    },
    Rule {
        id: "CompareObjectsWithEquals",
        class: Defect::WrongOperator,
        reason: "objects compared with == or != instead of equals()",
        off: Some(
            "precision 0/20 on the Java precision corpus (109 findings): intended identity comparisons (tree nodes, sentinels, this)",
        ),
    },
    Rule {
        id: "ComparisonWithNaN",
        class: Defect::WrongOperator,
        reason: "a comparison with NaN, always false",
        off: None,
    },
    Rule {
        id: "ConstructorCallsOverridableMethod",
        class: Defect::Uninitialised,
        reason: "a constructor that calls an overridable method, which sees a half-built object",
        off: Some(
            "precision 0/9 on the Java precision corpus (9 findings): no subclass in the project overrides the method",
        ),
    },
    Rule {
        id: "DoNotThrowExceptionInFinally",
        class: Defect::LanguageTrap,
        reason: "a throw in finally: it replaces the exception in flight",
        off: None,
    },
    Rule {
        id: "DoubleCheckedLocking",
        class: Defect::DataRace,
        reason: "double-checked locking without volatile: another thread can see a half-built object",
        off: None,
    },
    Rule {
        id: "EmptyCatchBlock",
        class: Defect::SwallowedError,
        reason: "an empty catch block (kept on: the swallowed-errors transformation reads it)",
        off: None,
    },
    Rule {
        id: "EqualsNull",
        class: Defect::WrongOperator,
        reason: "x.equals(null), always false or a NullPointerException",
        off: Some(
            "one finding on the Java precision corpus, too few to sample (0/1: a test of the equals contract); new to the list, so it stays off",
        ),
    },
    Rule {
        id: "IdempotentOperations",
        class: Defect::WrongOperator,
        reason: "an assignment of a variable to itself",
        off: None,
    },
    Rule {
        id: "ImplicitSwitchFallThrough",
        class: Defect::WrongOperator,
        reason: "a switch case that falls through to the next without a comment saying so",
        off: Some(
            "one finding on the Java precision corpus, too few to sample (0/1: a commented fall-through); new to the list, so it stays off",
        ),
    },
    Rule {
        id: "InvalidLogMessageFormat",
        class: Defect::WrongOperator,
        reason: "a log message whose placeholders do not match its arguments",
        off: None,
    },
    Rule {
        id: "JumbledIncrementer",
        class: Defect::WrongOperator,
        reason: "an inner loop that increments the outer loop's variable",
        off: None,
    },
    Rule {
        id: "MisplacedNullCheck",
        class: Defect::NullDereference,
        reason: "a null check after the value was already dereferenced in the same condition",
        off: None,
    },
    Rule {
        id: "NonThreadSafeSingleton",
        class: Defect::DataRace,
        reason: "a lazily created singleton without synchronisation",
        off: None,
    },
    Rule {
        id: "OverrideBothEqualsAndHashcode",
        class: Defect::WrongOperator,
        reason: "equals() without hashCode() or the reverse: hash collections misbehave",
        off: Some(
            "precision 0/5 on the Java precision corpus (5 findings): test-only value classes never put in a hash collection",
        ),
    },
    Rule {
        id: "ReturnFromFinallyBlock",
        class: Defect::LanguageTrap,
        reason: "return in finally: it discards the exception in flight",
        off: None,
    },
    Rule {
        id: "SuspiciousEqualsMethodName",
        class: Defect::WrongOperator,
        reason: "an equals or hashCode lookalike that overrides nothing",
        off: Some(
            "one finding on the Java precision corpus, too few to sample (0/1: a JUnit test named equals); new to the list, so it stays off",
        ),
    },
    Rule {
        id: "UnconditionalIfStatement",
        class: Defect::WrongOperator,
        reason: "an if whose condition is the constant true or false",
        off: None,
    },
    Rule {
        id: "UnsynchronizedStaticFormatter",
        class: Defect::DataRace,
        reason: "a static DateFormat or similar used from several threads without a lock",
        off: None,
    },
    Rule {
        id: "UnusedNullCheckInEquals",
        class: Defect::NullDereference,
        reason: "equals() called on the argument after the null check of the receiver it was meant for",
        off: Some(
            "one finding on the Java precision corpus, too few to sample (0/1: the receiver can never be null); new to the list, so it stays off",
        ),
    },
    Rule {
        id: "UselessOperationOnImmutable",
        class: Defect::WrongOperator,
        reason: "the result of an operation on an immutable BigDecimal or BigInteger thrown away",
        off: None,
    },
];

/// golangci-lint 2.14.0 with the image's `defects.yml`: one rule id per
/// linter, so govet runs only nilness.
pub(super) const GOLANGCI_DEFECT_RULES: &[Rule] = &[
    Rule {
        id: "bodyclose",
        class: Defect::ResourceLeak,
        reason: "an HTTP response body not closed",
        off: None,
    },
    Rule {
        id: "durationcheck",
        class: Defect::Overflow,
        reason: "two time.Duration values multiplied together",
        off: None,
    },
    Rule {
        id: "govet",
        class: Defect::NullDereference,
        reason: "go vet's nilness analyzer (the only one this run enables): a nil pointer dereferenced, or compared where it is always or never nil",
        off: None,
    },
    Rule {
        id: "makezero",
        class: Defect::WrongOperator,
        reason: "append to a slice made with a non-zero length, which keeps the zero values in front",
        off: None,
    },
    Rule {
        id: "nilerr",
        class: Defect::SwallowedError,
        reason: "nil returned where the error just checked is not nil, or the reverse",
        off: None,
    },
    Rule {
        id: "nilnesserr",
        class: Defect::SwallowedError,
        reason: "a different, nil error returned after an error check",
        off: None,
    },
    Rule {
        id: "rowserrcheck",
        class: Defect::UncheckedResult,
        reason: "sql.Rows.Err not checked after the loop",
        off: Some(
            "one finding on the Go precision corpus, too few to sample (1/1: rows.Err unchecked after the loop); new to the list, so it stays off",
        ),
    },
    Rule {
        id: "spancheck",
        class: Defect::ResourceLeak,
        reason: "an OpenTelemetry or OpenCensus span not ended",
        off: None,
    },
    Rule {
        id: "sqlclosecheck",
        class: Defect::ResourceLeak,
        reason: "sql.Rows or sql.Stmt not closed",
        off: Some(
            "4 findings on the Go precision corpus, too few to sample (4/4: prepared statements never closed); new to the list, so it stays off",
        ),
    },
];

/// Swift: the rules `swiftlint` 0.65.1 runs with `--only-rule`.
pub(super) const SWIFTLINT_DEFECT_RULES: &[Rule] = &[
    Rule {
        id: "discarded_notification_center_observer",
        class: Defect::ResourceLeak,
        reason: "a block-based observer token thrown away: it can never be removed",
        off: None,
    },
    Rule {
        id: "duplicate_conditions",
        class: Defect::WrongOperator,
        reason: "an if/else-if chain or switch that tests the same condition twice",
        off: None,
    },
    Rule {
        id: "force_cast",
        class: Defect::ForcedCrash,
        reason: "as!, which crashes when the cast fails",
        off: Some(
            "precision 0/20 on the Swift precision corpus (60 findings): test and demo code casting a value whose type it just created",
        ),
    },
    Rule {
        id: "force_try",
        class: Defect::ForcedCrash,
        reason: "try!, which crashes when the call throws",
        off: Some(
            "precision 0/20 on the Swift precision corpus (115 findings): test setup calls, where a throw is a test failure",
        ),
    },
    Rule {
        id: "force_unwrapping",
        class: Defect::NullDereference,
        reason: "a forced unwrap (!) of an optional, which crashes on nil",
        off: Some(
            "precision 0/20 on the Swift precision corpus (575 findings): test code, where a crash is a failure, and unwraps the code has just made safe",
        ),
    },
    Rule {
        id: "identical_operands",
        class: Defect::WrongOperator,
        reason: "the same operand on both sides of a comparison",
        off: None,
    },
    Rule {
        id: "implicitly_unwrapped_optional",
        class: Defect::NullDereference,
        reason: "an implicitly unwrapped optional (Type!), which crashes when read while nil",
        off: Some(
            "precision 0/20 on the Swift precision corpus (40 findings): fixtures and outlets set before any read",
        ),
    },
    Rule {
        id: "unhandled_throwing_task",
        class: Defect::UnobservedAsync,
        reason: "a Task whose thrown error nobody reads",
        off: None,
    },
    Rule {
        id: "unowned_variable_capture",
        class: Defect::UseAfterFree,
        reason: "an unowned capture, which crashes if the object is gone when the closure runs",
        off: Some(
            "precision 0/20 on the Swift precision corpus (39 findings): closures owned by the object they capture",
        ),
    },
    Rule {
        id: "unused_setter_value",
        class: Defect::WrongOperator,
        reason: "a setter that ignores newValue",
        off: Some(
            "precision 0/5 on the Swift precision corpus (5 findings): extension points whose setter is empty on purpose",
        ),
    },
    Rule {
        id: "weak_delegate",
        class: Defect::ResourceLeak,
        reason: "a strong delegate property: a retain cycle that leaks both objects",
        off: Some(
            "precision 0/8 on the Swift precision corpus (8 findings): owned delegate objects and closure wrappers, no cycle",
        ),
    },
];

/// SonarAnalyzer.CSharp 9.32.0.97167 and the .NET SDK analyzers: the rules
/// the image's `sloptopop.globalconfig` turns on. Their other rules pass
/// through untagged.
pub(super) const SONAR_CSHARP_RULES: &[Rule] = &[
    Rule {
        id: "CA1806",
        class: Defect::UncheckedResult,
        reason: "the result of a call (HRESULT, TryParse, a new object) thrown away",
        off: None,
    },
    Rule {
        id: "CA2000",
        class: Defect::ResourceLeak,
        reason: "an IDisposable not disposed before it goes out of scope",
        off: None,
    },
    Rule {
        id: "CA2011",
        class: Defect::WrongOperator,
        reason: "a property setter that assigns the property itself: infinite recursion",
        off: None,
    },
    Rule {
        id: "CA2012",
        class: Defect::UnobservedAsync,
        reason: "a ValueTask awaited twice, never, or read before it completed",
        off: None,
    },
    Rule {
        id: "CA2013",
        class: Defect::WrongOperator,
        reason: "ReferenceEquals on value types, always false",
        off: None,
    },
    Rule {
        id: "CA2017",
        class: Defect::WrongOperator,
        reason: "a logging message template whose placeholders do not match its arguments",
        off: None,
    },
    Rule {
        id: "CA2018",
        class: Defect::OutOfBounds,
        reason: "Buffer.BlockCopy given an element count instead of a byte count",
        off: None,
    },
    Rule {
        id: "CA2021",
        class: Defect::WrongOperator,
        reason: "Cast<T> or OfType<T> with a type the elements can never be",
        off: None,
    },
    Rule {
        id: "CA2213",
        class: Defect::ResourceLeak,
        reason: "a disposable field the type's Dispose does not dispose",
        off: None,
    },
    Rule {
        id: "CA2241",
        class: Defect::WrongOperator,
        reason: "a format string whose placeholders do not match its arguments",
        off: None,
    },
    Rule {
        id: "CA2242",
        class: Defect::WrongOperator,
        reason: "a comparison with NaN, always false",
        off: None,
    },
    Rule {
        id: "CA2245",
        class: Defect::WrongOperator,
        reason: "a property assigned to itself",
        off: None,
    },
    Rule {
        id: "CA2248",
        class: Defect::WrongOperator,
        reason: "Enum.HasFlag with a value of another enum type",
        off: None,
    },
    Rule {
        id: "S1764",
        class: Defect::WrongOperator,
        reason: "the same expression on both sides of a binary operator",
        off: None,
    },
    Rule {
        id: "S2077",
        class: Defect::Injection,
        reason: "SQL built by string formatting or concatenation",
        off: None,
    },
    Rule {
        id: "S2201",
        class: Defect::UncheckedResult,
        reason: "the result of a call without side effects thrown away",
        off: None,
    },
    Rule {
        id: "S2222",
        class: Defect::LockNotReleased,
        reason: "a lock acquired and not released on every path",
        off: None,
    },
    Rule {
        id: "S2259",
        class: Defect::NullDereference,
        reason: "a null reference dereferenced on some path",
        off: None,
    },
    Rule {
        id: "S2583",
        class: Defect::WrongOperator,
        reason: "a condition that is always true or always false, so a branch is dead",
        off: None,
    },
    Rule {
        id: "S2589",
        class: Defect::WrongOperator,
        reason: "a boolean expression whose value is always the same",
        off: None,
    },
    Rule {
        id: "S2930",
        class: Defect::ResourceLeak,
        reason: "an IDisposable created and never disposed",
        off: None,
    },
    Rule {
        id: "S3655",
        class: Defect::NullDereference,
        reason: "Nullable<T>.Value read where it can be empty",
        off: None,
    },
    Rule {
        id: "S3966",
        class: Defect::UseAfterFree,
        reason: "an object disposed twice",
        off: None,
    },
];

/// The `eslint` 9.39.5 core rules in the image's configuration that need no
/// type information. Its typescript-eslint rules pass through untagged.
pub(super) const ESLINT_TYPESCRIPT_RULES: &[Rule] = &[
    Rule {
        id: "array-callback-return",
        class: Defect::WrongOperator,
        reason: "an array method callback (map, filter, reduce…) that does not return",
        off: Some(
            "4 findings on the JavaScript/TypeScript precision corpus, too few to sample (0/4: the missing return is the intended undefined); new to the list, so it stays off",
        ),
    },
    Rule {
        id: "for-direction",
        class: Defect::OffByOne,
        reason: "a for loop whose counter moves away from its end condition",
        off: None,
    },
    Rule {
        id: "getter-return",
        class: Defect::WrongOperator,
        reason: "a getter that does not return",
        off: None,
    },
    Rule {
        id: "no-async-promise-executor",
        class: Defect::UnobservedAsync,
        reason: "an async Promise executor, whose thrown errors the promise never sees",
        off: None,
    },
    Rule {
        id: "no-compare-neg-zero",
        class: Defect::WrongOperator,
        reason: "a comparison with -0, which equals 0",
        off: None,
    },
    Rule {
        id: "no-cond-assign",
        class: Defect::WrongOperator,
        reason: "an assignment in a condition",
        off: None,
    },
    Rule {
        id: "no-constant-binary-expression",
        class: Defect::WrongOperator,
        reason: "a comparison or logical expression whose result never changes",
        off: Some(
            "4 findings on the JavaScript/TypeScript precision corpus, too few to sample (2/4); new to the list, so it stays off",
        ),
    },
    Rule {
        id: "no-constant-condition",
        class: Defect::WrongOperator,
        reason: "a condition that is a constant",
        off: None,
    },
    Rule {
        id: "no-dupe-else-if",
        class: Defect::WrongOperator,
        reason: "an else-if that repeats an earlier condition of the chain",
        off: None,
    },
    Rule {
        id: "no-duplicate-case",
        class: Defect::WrongOperator,
        reason: "a switch with the same case twice",
        off: None,
    },
    Rule {
        id: "no-empty",
        class: Defect::SwallowedError,
        reason: "an empty block, an empty catch among them (kept on: the swallowed-errors transformation reads it)",
        off: None,
    },
    Rule {
        id: "no-fallthrough",
        class: Defect::WrongOperator,
        reason: "a switch case that falls through without a comment saying so",
        off: None,
    },
    Rule {
        id: "no-loop-func",
        class: Defect::LoopCapture,
        reason: "a function created in a loop that captures a variable the loop changes",
        off: Some(
            "4 findings on the JavaScript/TypeScript precision corpus, too few to sample (0/4: closures run within the same iteration); new to the list, so it stays off",
        ),
    },
    Rule {
        id: "no-loss-of-precision",
        class: Defect::Overflow,
        reason: "a number literal that cannot be stored exactly as a double",
        off: None,
    },
    Rule {
        id: "no-promise-executor-return",
        class: Defect::UnobservedAsync,
        reason: "a value returned from a Promise executor, which is ignored",
        off: Some(
            "precision 0/20 on the JavaScript/TypeScript precision corpus (26 findings): arrow executors that return setTimeout's handle by syntax only",
        ),
    },
    Rule {
        id: "no-self-assign",
        class: Defect::WrongOperator,
        reason: "a variable assigned to itself",
        off: None,
    },
    Rule {
        id: "no-self-compare",
        class: Defect::WrongOperator,
        reason: "a value compared with itself",
        off: None,
    },
    Rule {
        id: "no-unmodified-loop-condition",
        class: Defect::WrongOperator,
        reason: "a loop condition whose variables the body never changes",
        off: None,
    },
    Rule {
        id: "no-unsafe-finally",
        class: Defect::LanguageTrap,
        reason: "return, throw, break or continue in finally: it overrides the try's outcome",
        off: None,
    },
    Rule {
        id: "no-unsafe-negation",
        class: Defect::WrongOperator,
        reason: "! applied to the left operand of in or instanceof",
        off: None,
    },
    Rule {
        id: "no-unsafe-optional-chaining",
        class: Defect::NullDereference,
        reason: "an optional chain whose undefined result is then used where it throws",
        off: None,
    },
    Rule {
        id: "require-atomic-updates",
        class: Defect::DataRace,
        reason: "a variable updated from a value read before an await: another update in between is lost",
        off: Some(
            "precision 0/20 on the JavaScript/TypeScript precision corpus (33 findings): writes to per-request or per-test objects and restores of saved values",
        ),
    },
    Rule {
        id: "use-isnan",
        class: Defect::WrongOperator,
        reason: "a comparison with NaN, always false",
        off: None,
    },
    Rule {
        id: "valid-typeof",
        class: Defect::WrongOperator,
        reason: "typeof compared with a string no type has",
        off: None,
    },
];

/// detekt 1.23.8: the rules the image's `sloptopop.yml` turns on (it runs
/// without type resolution, so the rules that need it report nothing).
pub(super) const DETEKT_RULES: &[Rule] = &[
    Rule {
        id: "detekt.empty-blocks.EmptyCatchBlock",
        class: Defect::SwallowedError,
        reason: "an empty catch block (kept on: the swallowed-errors transformation reads it)",
        off: None,
    },
    Rule {
        id: "detekt.exceptions.ReturnFromFinally",
        class: Defect::LanguageTrap,
        reason: "return in finally: it discards the exception in flight",
        off: None,
    },
    Rule {
        id: "detekt.exceptions.ThrowingExceptionFromFinally",
        class: Defect::LanguageTrap,
        reason: "a throw in finally: it replaces the exception in flight",
        off: Some(
            "one finding on the Kotlin precision corpus, too few to sample (1/1); new to the list, so it stays off",
        ),
    },
    Rule {
        id: "detekt.potential-bugs.AvoidReferentialEquality",
        class: Defect::WrongOperator,
        reason: "=== or !== on a type whose instances should be compared with ==",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.CastNullableToNonNullableType",
        class: Defect::NullDereference,
        reason: "a nullable value cast to a non-nullable type, which throws on null",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.EqualsAlwaysReturnsTrueOrFalse",
        class: Defect::WrongOperator,
        reason: "an equals() that always returns the same value",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.IgnoredReturnValue",
        class: Defect::UncheckedResult,
        reason: "the result of a function annotated @CheckReturnValue thrown away",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.InvalidRange",
        class: Defect::OffByOne,
        reason: "a range that is empty because its bounds are the wrong way round",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.IteratorHasNextCallsNextMethod",
        class: Defect::WrongOperator,
        reason: "an Iterator.hasNext() that calls next() and so moves the iterator",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.LateinitUsage",
        class: Defect::Uninitialised,
        reason: "a lateinit property, which throws when read before it is set",
        off: Some(
            "precision 0/20 on the Kotlin precision corpus (58 findings): test fixtures and lifecycle properties set before any read",
        ),
    },
    Rule {
        id: "detekt.potential-bugs.MapGetWithNotNullAssertionOperator",
        class: Defect::NullDereference,
        reason: "map[key]!!, which throws when the key is missing",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.NullCheckOnMutableProperty",
        class: Defect::DataRace,
        reason: "a null check on a var property that another thread can change before the use",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.PropertyUsedBeforeDeclaration",
        class: Defect::Uninitialised,
        reason: "a property read in an initializer before the property is declared",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.UnconditionalJumpStatementInLoop",
        class: Defect::WrongOperator,
        reason: "a loop whose body always returns or breaks: it runs at most once",
        off: Some(
            "3 findings on the Kotlin precision corpus, too few to sample (0/3: the jumps are conditional); new to the list, so it stays off",
        ),
    },
    Rule {
        id: "detekt.potential-bugs.UnreachableCatchBlock",
        class: Defect::WrongOperator,
        reason: "a catch block an earlier catch of a wider type always takes first",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.UnreachableCode",
        class: Defect::WrongOperator,
        reason: "code after a return, throw, break or continue",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.UnsafeCallOnNullableType",
        class: Defect::NullDereference,
        reason: "!! on a nullable value, which throws on null",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.UnsafeCast",
        class: Defect::ForcedCrash,
        reason: "a cast that can never succeed, so it always throws",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.UnusedUnaryOperator",
        class: Defect::WrongOperator,
        reason: "a unary + or - whose result is not used (a broken line continuation)",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.UselessPostfixExpression",
        class: Defect::WrongOperator,
        reason: "a postfix ++ or -- whose effect is lost (return x++)",
        off: None,
    },
    Rule {
        id: "detekt.potential-bugs.WrongEqualsTypeParameter",
        class: Defect::WrongOperator,
        reason: "an equals whose parameter is not Any?, so it overloads instead of overriding",
        off: None,
    },
];

/// eslint-plugin-sonarjs 2.0.4: the defect rules of its recommended set
/// that need no type information (reported before they were listed;
/// tagged, and they stay on). Its other rules pass through untagged; `null-dereference`
/// and the other type-aware rules turn themselves off without types.
pub(super) const SONARJS_RULES: &[Rule] = &[
    Rule {
        id: "sonarjs/bitwise-operators",
        class: Defect::WrongOperator,
        reason: "a bitwise & or | where the logical && or || was meant",
        off: None,
    },
    Rule {
        id: "sonarjs/misplaced-loop-counter",
        class: Defect::WrongOperator,
        reason: "a for loop that updates a counter other than the one its condition tests",
        off: None,
    },
    Rule {
        id: "sonarjs/no-all-duplicated-branches",
        class: Defect::WrongOperator,
        reason: "an if/else or switch whose branches are all the same",
        off: None,
    },
    Rule {
        id: "sonarjs/no-case-label-in-switch",
        class: Defect::WrongOperator,
        reason: "a label inside a switch that looks like a case",
        off: None,
    },
    Rule {
        id: "sonarjs/no-dead-store",
        class: Defect::WrongOperator,
        reason: "a value assigned and overwritten or dropped before anyone reads it",
        off: None,
    },
    Rule {
        id: "sonarjs/no-duplicated-branches",
        class: Defect::WrongOperator,
        reason: "two branches of an if chain or switch with the same body",
        off: None,
    },
    Rule {
        id: "sonarjs/no-element-overwrite",
        class: Defect::WrongOperator,
        reason: "an array element or map entry written twice in a row",
        off: None,
    },
    Rule {
        id: "sonarjs/no-equals-in-for-termination",
        class: Defect::OffByOne,
        reason: "a for loop that ends on == or !=, which a step past the end never meets",
        off: None,
    },
    Rule {
        id: "sonarjs/no-gratuitous-expressions",
        class: Defect::WrongOperator,
        reason: "a condition whose value is already known at that point",
        off: None,
    },
    Rule {
        id: "sonarjs/no-identical-conditions",
        class: Defect::WrongOperator,
        reason: "an if chain that tests the same condition twice",
        off: None,
    },
    Rule {
        id: "sonarjs/no-identical-expressions",
        class: Defect::WrongOperator,
        reason: "the same expression on both sides of an operator",
        off: None,
    },
    Rule {
        id: "sonarjs/no-ignored-exceptions",
        class: Defect::SwallowedError,
        reason: "an empty catch block that drops the exception",
        off: None,
    },
    Rule {
        id: "sonarjs/no-invariant-returns",
        class: Defect::WrongOperator,
        reason: "a function that returns the same value on every path",
        off: None,
    },
    Rule {
        id: "sonarjs/no-os-command-from-path",
        class: Defect::Injection,
        reason: "an OS command run by a name looked up in PATH, which the environment controls",
        off: None,
    },
    Rule {
        id: "sonarjs/no-redundant-assignments",
        class: Defect::WrongOperator,
        reason: "an assignment of the value the variable already holds",
        off: None,
    },
    Rule {
        id: "sonarjs/no-same-line-conditional",
        class: Defect::WrongOperator,
        reason: "an if on the same line as the closing brace of another, a missing else",
        off: None,
    },
    Rule {
        id: "sonarjs/no-self-compare",
        class: Defect::WrongOperator,
        reason: "a value compared with itself",
        off: None,
    },
    Rule {
        id: "sonarjs/no-unenclosed-multiline-block",
        class: Defect::WrongOperator,
        reason: "indented lines after an if without braces, of which only the first is conditional",
        off: None,
    },
    Rule {
        id: "sonarjs/no-unthrown-error",
        class: Defect::WrongOperator,
        reason: "an Error created and never thrown",
        off: None,
    },
    Rule {
        id: "sonarjs/no-unused-collection",
        class: Defect::WrongOperator,
        reason: "a collection filled and never read",
        off: None,
    },
    Rule {
        id: "sonarjs/no-unused-expressions",
        class: Defect::WrongOperator,
        reason: "an expression statement with no effect",
        off: None,
    },
    Rule {
        id: "sonarjs/no-use-of-empty-return-value",
        class: Defect::WrongOperator,
        reason: "the result of a function that returns nothing used as a value",
        off: None,
    },
    Rule {
        id: "sonarjs/no-useless-increment",
        class: Defect::WrongOperator,
        reason: "x = x++, whose increment is lost",
        off: None,
    },
    Rule {
        id: "sonarjs/non-existent-operator",
        class: Defect::WrongOperator,
        reason: "=+, =- or =! where +=, -= or != was meant",
        off: None,
    },
    Rule {
        id: "sonarjs/os-command",
        class: Defect::Injection,
        reason: "an OS command built from values that may come from outside",
        off: None,
    },
    Rule {
        id: "sonarjs/sql-queries",
        class: Defect::Injection,
        reason: "SQL built by string formatting or concatenation",
        off: None,
    },
    Rule {
        id: "sonarjs/updated-loop-counter",
        class: Defect::WrongOperator,
        reason: "a for loop counter changed inside the body",
        off: None,
    },
];

/// Biome 2.5.15: the defect rules of its recommended set (reported before
/// they were listed; tagged, and they stay on). Its other rules pass through
/// untagged.
pub(super) const BIOME_RULES: &[Rule] = &[
    Rule {
        id: "lint/correctness/noConstantCondition",
        class: Defect::WrongOperator,
        reason: "a condition that is a constant",
        off: None,
    },
    Rule {
        id: "lint/correctness/noInvalidUseBeforeDeclaration",
        class: Defect::Uninitialised,
        reason: "a variable or parameter used before it is declared",
        off: None,
    },
    Rule {
        id: "lint/correctness/noPrecisionLoss",
        class: Defect::Overflow,
        reason: "a number literal that cannot be stored exactly as a double",
        off: None,
    },
    Rule {
        id: "lint/correctness/noSelfAssign",
        class: Defect::WrongOperator,
        reason: "a variable assigned to itself",
        off: None,
    },
    Rule {
        id: "lint/correctness/noSwitchDeclarations",
        class: Defect::WrongOperator,
        reason: "a declaration in a switch case that the other cases can see uninitialised",
        off: None,
    },
    Rule {
        id: "lint/correctness/noUnreachable",
        class: Defect::WrongOperator,
        reason: "code that can never run",
        off: None,
    },
    Rule {
        id: "lint/correctness/noUnsafeFinally",
        class: Defect::LanguageTrap,
        reason: "return, throw, break or continue in finally: it overrides the try's outcome",
        off: None,
    },
    Rule {
        id: "lint/correctness/noUnsafeOptionalChaining",
        class: Defect::NullDereference,
        reason: "an optional chain whose undefined result is then used where it throws",
        off: None,
    },
    Rule {
        id: "lint/correctness/useIsNan",
        class: Defect::WrongOperator,
        reason: "a comparison with NaN, always false",
        off: None,
    },
    Rule {
        id: "lint/correctness/useValidTypeof",
        class: Defect::WrongOperator,
        reason: "typeof compared with a string no type has",
        off: None,
    },
    Rule {
        id: "lint/suspicious/noAssignInExpressions",
        class: Defect::WrongOperator,
        reason: "an assignment inside an expression, often a mistyped comparison",
        off: None,
    },
    Rule {
        id: "lint/suspicious/noAsyncPromiseExecutor",
        class: Defect::UnobservedAsync,
        reason: "an async Promise executor, whose thrown errors the promise never sees",
        off: None,
    },
    Rule {
        id: "lint/suspicious/noCompareNegZero",
        class: Defect::WrongOperator,
        reason: "a comparison with -0, which equals 0",
        off: None,
    },
    Rule {
        id: "lint/suspicious/noDoubleEquals",
        class: Defect::LanguageTrap,
        reason: "== or != with its type coercion",
        off: None,
    },
    Rule {
        id: "lint/suspicious/noDuplicateCase",
        class: Defect::WrongOperator,
        reason: "a switch with the same case twice",
        off: None,
    },
    Rule {
        id: "lint/suspicious/noDuplicateElseIf",
        class: Defect::WrongOperator,
        reason: "an else-if that repeats an earlier condition of the chain",
        off: None,
    },
    Rule {
        id: "lint/suspicious/noDuplicateObjectKeys",
        class: Defect::WrongOperator,
        reason: "an object literal with the same key twice: the first value is lost",
        off: None,
    },
    Rule {
        id: "lint/suspicious/noFallthroughSwitchClause",
        class: Defect::WrongOperator,
        reason: "a switch case that falls through to the next",
        off: None,
    },
    Rule {
        id: "lint/suspicious/noGlobalIsNan",
        class: Defect::LanguageTrap,
        reason: "the global isNaN, which coerces its argument",
        off: None,
    },
    Rule {
        id: "lint/suspicious/noSelfCompare",
        class: Defect::WrongOperator,
        reason: "a value compared with itself",
        off: None,
    },
    Rule {
        id: "lint/suspicious/noSparseArray",
        class: Defect::WrongOperator,
        reason: "an array literal with a hole (two commas in a row)",
        off: None,
    },
    Rule {
        id: "lint/suspicious/noUnsafeNegation",
        class: Defect::WrongOperator,
        reason: "! applied to the left operand of in or instanceof",
        off: None,
    },
    Rule {
        id: "lint/suspicious/useIterableCallbackReturn",
        class: Defect::WrongOperator,
        reason: "an array method callback (map, filter, every…) that does not return a value",
        off: None,
    },
];
