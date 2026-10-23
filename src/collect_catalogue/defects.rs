//! The defect rules of the C and C++ analyzers: every clang-tidy check and
//! Cppcheck id `collect` reports, each with its defect class and what it
//! finds. A rule the precision study found below the budget (sample
//! precision below 0.80 on the C/C++ precision corpus, for a rule with five
//! findings or more there) stays listed, `off`, with the reason: clang-tidy
//! does not run it, and Cppcheck's findings of it are dropped. A rule with
//! fewer findings keeps its earlier default; one with none stays on until
//! measured.
//! `docs/collect.md` lists them.
//!
//! Standards: SCF TDA-17.1 (Static Code Analysis: each defect rule with its class),
//! VPM-12.2 (Breadth / Depth of Coverage For Vulnerability Scanning).

use crate::collect_convert::{Defect, Rule};

/// The clang-tidy `-checks` argument: no default checks, then every
/// reported check of [`CLANG_TIDY_RULES`] (the catalogue tests keep the
/// two equal). The Clang Static Analyzer's modeling checkers, which report
/// nothing themselves, are enabled with the checkers that depend on them.
pub(super) const CLANG_TIDY_CHECKS: &str = "-*,bugprone-dangling-handle,bugprone-empty-catch,bugprone-misplaced-widening-cast,bugprone-too-small-loop-variable,bugprone-unchecked-optional-access,bugprone-unused-return-value,bugprone-use-after-move,clang-analyzer-core.BitwiseShift,clang-analyzer-core.CallAndMessage,clang-analyzer-core.DivideZero,clang-analyzer-core.NonNullParamChecker,clang-analyzer-core.NullDereference,clang-analyzer-core.StackAddressEscape,clang-analyzer-core.UndefinedBinaryOperatorResult,clang-analyzer-core.VLASize,clang-analyzer-core.uninitialized.ArraySubscript,clang-analyzer-core.uninitialized.Assign,clang-analyzer-core.uninitialized.Branch,clang-analyzer-core.uninitialized.CapturedBlockVariable,clang-analyzer-core.uninitialized.NewArraySize,clang-analyzer-core.uninitialized.UndefReturn,clang-analyzer-cplusplus.ArrayDelete,clang-analyzer-cplusplus.InnerPointer,clang-analyzer-cplusplus.Move,clang-analyzer-cplusplus.NewDelete,clang-analyzer-cplusplus.NewDeleteLeaks,clang-analyzer-cplusplus.PlacementNew,clang-analyzer-cplusplus.PureVirtualCall,clang-analyzer-cplusplus.StringChecker,clang-analyzer-unix.API,clang-analyzer-unix.BlockInCriticalSection,clang-analyzer-unix.Errno,clang-analyzer-unix.MallocSizeof,clang-analyzer-unix.MismatchedDeallocator,clang-analyzer-unix.StdCLibraryFunctions,clang-analyzer-unix.Vfork,clang-analyzer-unix.cstring.BadSizeArg,clang-analyzer-unix.cstring.NullArg";

/// clang-tidy 19.1.0: the `clang-analyzer-core`, `-cplusplus`, `-deadcode`
/// and `-unix` families (without `core.NonnilStringConstants`, which is
/// Objective-C only, and the modeling checkers), and the bugprone checks
/// for the same classes.
pub(super) const CLANG_TIDY_RULES: &[Rule] = &[
    Rule {
        id: "bugprone-branch-clone",
        class: Defect::WrongOperator,
        reason: "identical branches of an if/else chain or switch: a copy-paste or wrong-branch error",
        off: Some(
            "precision 0/20 on the C/C++ precision corpus (29 findings, with the image's system headers): the shared branch bodies are deliberate, and three \"identical\" verdicts come from bodies clang could not type-check",
        ),
    },
    Rule {
        id: "bugprone-dangling-handle",
        class: Defect::UseAfterFree,
        reason: "a string_view or other non-owning handle that outlives the object it points into",
        off: None,
    },
    Rule {
        id: "bugprone-empty-catch",
        class: Defect::SwallowedError,
        reason: "an empty catch handler hides the error (the swallowed-errors transformation)",
        off: None,
    },
    Rule {
        id: "bugprone-implicit-widening-of-multiplication-result",
        class: Defect::Overflow,
        reason: "a multiplication done in the narrow type, then widened: the product can overflow first",
        off: Some(
            "precision 1/20 on the C/C++ precision corpus (22 findings, with the image's system headers): products of small constants or bounded values that fit in int",
        ),
    },
    Rule {
        id: "bugprone-infinite-loop",
        class: Defect::WrongOperator,
        reason: "a loop whose condition no statement in its body can change",
        off: Some(
            "one finding on the C/C++ precision corpus with the image's system headers (0/1: both loop variables change in the body), too few to overturn the earlier 0/14",
        ),
    },
    Rule {
        id: "bugprone-misplaced-widening-cast",
        class: Defect::Overflow,
        reason: "a cast that widens the result of a calculation that already overflowed",
        off: None,
    },
    Rule {
        id: "bugprone-narrowing-conversions",
        class: Defect::Overflow,
        reason: "an implicit conversion that can lose value (integer or floating point)",
        off: Some(
            "precision 0/20 on the C/C++ precision corpus (246 findings, with the image's system headers): values that fit the target type in every reachable case",
        ),
    },
    Rule {
        id: "bugprone-signed-char-misuse",
        class: Defect::Overflow,
        reason: "a signed char converted to a wider integer: negative values sign-extend",
        off: Some(
            "precision 1/10 on the C/C++ precision corpus (10 findings, with the image's system headers): the converted values are ASCII, small table entries or bytes read as signed on purpose",
        ),
    },
    Rule {
        id: "bugprone-too-small-loop-variable",
        class: Defect::OffByOne,
        reason: "a loop variable whose type cannot reach the loop bound",
        off: None,
    },
    Rule {
        id: "bugprone-unchecked-optional-access",
        class: Defect::NullDereference,
        reason: "std::optional (and absl/folly) value read without checking it holds one",
        off: None,
    },
    Rule {
        id: "bugprone-unused-return-value",
        class: Defect::UncheckedResult,
        reason: "the result of a function whose result must be used is dropped",
        off: None,
    },
    Rule {
        id: "bugprone-use-after-move",
        class: Defect::UseAfterFree,
        reason: "an object used after it was moved from",
        off: None,
    },
    Rule {
        id: "clang-analyzer-core.BitwiseShift",
        class: Defect::Overflow,
        reason: "a shift by a negative amount or by at least the operand's width (undefined behaviour)",
        off: None,
    },
    Rule {
        id: "clang-analyzer-core.CallAndMessage",
        class: Defect::NullDereference,
        reason: "a call through a null function pointer or on a null object, or with an undefined argument",
        off: None,
    },
    Rule {
        id: "clang-analyzer-core.DivideZero",
        class: Defect::DivisionByZero,
        reason: "a division or remainder whose divisor is zero on some path",
        off: None,
    },
    Rule {
        id: "clang-analyzer-core.NonNullParamChecker",
        class: Defect::NullDereference,
        reason: "null passed for a parameter declared nonnull",
        off: None,
    },
    Rule {
        id: "clang-analyzer-core.NullDereference",
        class: Defect::NullDereference,
        reason: "a pointer that is null on some path is dereferenced",
        off: None,
    },
    Rule {
        id: "clang-analyzer-core.StackAddressEscape",
        class: Defect::UseAfterFree,
        reason: "the address of a local variable escapes its function",
        off: None,
    },
    Rule {
        id: "clang-analyzer-core.UndefinedBinaryOperatorResult",
        class: Defect::Uninitialised,
        reason: "an operator with an undefined (uninitialised) operand",
        off: None,
    },
    Rule {
        id: "clang-analyzer-core.VLASize",
        class: Defect::OutOfBounds,
        reason: "a variable-length array of zero, negative or undefined size",
        off: None,
    },
    Rule {
        id: "clang-analyzer-core.uninitialized.ArraySubscript",
        class: Defect::Uninitialised,
        reason: "an array indexed with an uninitialised value",
        off: None,
    },
    Rule {
        id: "clang-analyzer-core.uninitialized.Assign",
        class: Defect::Uninitialised,
        reason: "an uninitialised value assigned",
        off: None,
    },
    Rule {
        id: "clang-analyzer-core.uninitialized.Branch",
        class: Defect::Uninitialised,
        reason: "a branch on an uninitialised value",
        off: None,
    },
    Rule {
        id: "clang-analyzer-core.uninitialized.CapturedBlockVariable",
        class: Defect::Uninitialised,
        reason: "an uninitialised variable captured by a block",
        off: None,
    },
    Rule {
        id: "clang-analyzer-core.uninitialized.NewArraySize",
        class: Defect::Uninitialised,
        reason: "new[] with an uninitialised size",
        off: None,
    },
    Rule {
        id: "clang-analyzer-core.uninitialized.UndefReturn",
        class: Defect::Uninitialised,
        reason: "an uninitialised value returned",
        off: None,
    },
    Rule {
        id: "clang-analyzer-cplusplus.ArrayDelete",
        class: Defect::UseAfterFree,
        reason: "an array of derived objects deleted through a base-class pointer",
        off: None,
    },
    Rule {
        id: "clang-analyzer-cplusplus.InnerPointer",
        class: Defect::UseAfterFree,
        reason: "a pointer into a std::string's buffer used after the string changed or died",
        off: None,
    },
    Rule {
        id: "clang-analyzer-cplusplus.Move",
        class: Defect::UseAfterFree,
        reason: "an object used after it was moved from (path-sensitive)",
        off: None,
    },
    Rule {
        id: "clang-analyzer-cplusplus.NewDelete",
        class: Defect::UseAfterFree,
        reason: "use after delete, double delete, delete of memory new did not allocate",
        off: None,
    },
    Rule {
        id: "clang-analyzer-cplusplus.NewDeleteLeaks",
        class: Defect::ResourceLeak,
        reason: "memory from new never deleted on some path",
        off: None,
    },
    Rule {
        id: "clang-analyzer-cplusplus.PlacementNew",
        class: Defect::OutOfBounds,
        reason: "placement new into storage too small for the object",
        off: None,
    },
    Rule {
        id: "clang-analyzer-cplusplus.PureVirtualCall",
        class: Defect::UseAfterFree,
        reason: "a pure virtual call during construction or destruction: the object is used outside its lifetime as that type",
        off: None,
    },
    Rule {
        id: "clang-analyzer-cplusplus.StringChecker",
        class: Defect::NullDereference,
        reason: "a std::string constructed from a null pointer",
        off: None,
    },
    Rule {
        id: "clang-analyzer-deadcode.DeadStores",
        class: Defect::WrongOperator,
        reason: "a value stored and never read: a lost assignment or the wrong variable",
        off: Some(
            "precision 6/13 (0.46) on the C/C++ precision corpus (13 findings, with the image's system headers): the hits are overwritten error codes, the misses harmless last increments and results discarded on purpose",
        ),
    },
    Rule {
        id: "clang-analyzer-unix.API",
        class: Defect::WrongOperator,
        reason: "a POSIX call made wrongly: open with O_CREAT and no mode, an allocation of 0 bytes, pthread_once on a local",
        off: None,
    },
    Rule {
        id: "clang-analyzer-unix.BlockInCriticalSection",
        class: Defect::Deadlock,
        reason: "a blocking call made while a lock is held",
        off: None,
    },
    Rule {
        id: "clang-analyzer-unix.Errno",
        class: Defect::UncheckedResult,
        reason: "errno read when the last call did not set it, or not checked when it did",
        off: None,
    },
    Rule {
        id: "clang-analyzer-unix.Malloc",
        class: Defect::ResourceLeak,
        reason: "malloc'd memory leaked on some path (the same id also reports use after free and double free)",
        off: Some(
            "precision 2/7 on the C/C++ precision corpus (7 findings, with the image's system headers): most are tests whose allocator fails on purpose",
        ),
    },
    Rule {
        id: "clang-analyzer-unix.MallocSizeof",
        class: Defect::OutOfBounds,
        reason: "an allocation sized with sizeof of the wrong type",
        off: None,
    },
    Rule {
        id: "clang-analyzer-unix.MismatchedDeallocator",
        class: Defect::UseAfterFree,
        reason: "memory released by a deallocator that does not match its allocator",
        off: None,
    },
    Rule {
        id: "clang-analyzer-unix.StdCLibraryFunctions",
        class: Defect::NullDereference,
        reason: "a C library call that breaks the function's preconditions (mostly a null argument)",
        off: None,
    },
    Rule {
        id: "clang-analyzer-unix.Stream",
        class: Defect::ResourceLeak,
        reason: "a FILE stream leaked, used after fclose, or used when fopen failed",
        off: Some(
            "precision 1/6 on the C/C++ precision corpus (6 findings, with the image's system headers): idiomatic fread loops that stop at end of file",
        ),
    },
    Rule {
        id: "clang-analyzer-unix.Vfork",
        class: Defect::WrongOperator,
        reason: "a vfork child that does more than exec or _exit",
        off: None,
    },
    Rule {
        id: "clang-analyzer-unix.cstring.BadSizeArg",
        class: Defect::OutOfBounds,
        reason: "a strncat-style size argument that can overflow the destination",
        off: None,
    },
    Rule {
        id: "clang-analyzer-unix.cstring.NullArg",
        class: Defect::NullDereference,
        reason: "a null pointer passed to a C string function",
        off: None,
    },
];

/// Cppcheck 2.17.1, run with `--enable=warning,performance,portability`
/// and the test-assertion libraries (`googletest` and the image's
/// `tests.cfg`): the ids of the same classes; its other findings
/// (performance and portability advice, configuration and syntax
/// diagnostics) are dropped.
pub(super) const CPPCHECK_RULES: &[Rule] = &[
    Rule {
        id: "accessForwarded",
        class: Defect::UseAfterFree,
        reason: "a variable used after it was forwarded",
        off: None,
    },
    Rule {
        id: "accessMoved",
        class: Defect::UseAfterFree,
        reason: "a variable used after it was moved from",
        off: None,
    },
    Rule {
        id: "argumentSize",
        class: Defect::OutOfBounds,
        reason: "a buffer smaller than the size the called function expects",
        off: None,
    },
    Rule {
        id: "arrayIndexOutOfBounds",
        class: Defect::OutOfBounds,
        reason: "an array indexed past its end",
        off: None,
    },
    Rule {
        id: "arrayIndexOutOfBoundsCond",
        class: Defect::OffByOne,
        reason: "an array indexed past its end when a condition (usually the loop bound) holds",
        off: None,
    },
    Rule {
        id: "autoVariables",
        class: Defect::UseAfterFree,
        reason: "the address of a local variable stored where it outlives the function",
        off: None,
    },
    Rule {
        id: "autovarInvalidDeallocation",
        class: Defect::UseAfterFree,
        reason: "free or delete of a local (stack) variable",
        off: None,
    },
    Rule {
        id: "badBitmaskCheck",
        class: Defect::WrongOperator,
        reason: "a bitmask test that is always true or always false (| for &)",
        off: None,
    },
    Rule {
        id: "bufferAccessOutOfBounds",
        class: Defect::OutOfBounds,
        reason: "a buffer read or written past its end",
        off: None,
    },
    Rule {
        id: "charLiteralWithCharPtrCompare",
        class: Defect::WrongOperator,
        reason: "a char literal compared with a pointer: a missing dereference",
        off: None,
    },
    Rule {
        id: "checkCastIntToCharAndBack",
        class: Defect::Overflow,
        reason: "a function's int result (EOF) stored in a char, then compared with EOF",
        off: None,
    },
    Rule {
        id: "compareBoolExpressionWithInt",
        class: Defect::WrongOperator,
        reason: "a boolean expression compared with an integer other than 0 or 1",
        off: None,
    },
    Rule {
        id: "comparePointers",
        class: Defect::WrongOperator,
        reason: "pointers to different objects compared",
        off: None,
    },
    Rule {
        id: "comparisonOfBoolWithInvalidComparator",
        class: Defect::WrongOperator,
        reason: "a boolean compared with <, >, <= or >=",
        off: None,
    },
    Rule {
        id: "containerOutOfBounds",
        class: Defect::OutOfBounds,
        reason: "a container indexed past its end",
        off: None,
    },
    Rule {
        id: "ctuArrayIndex",
        class: Defect::OutOfBounds,
        reason: "an array indexed past its end, across functions",
        off: None,
    },
    Rule {
        id: "ctuPointerArith",
        class: Defect::OutOfBounds,
        reason: "pointer arithmetic past an object's end, across functions",
        off: None,
    },
    Rule {
        id: "ctunullpointer",
        class: Defect::NullDereference,
        reason: "a null pointer dereferenced, across functions",
        off: Some(
            "precision 0/3 on the C/C++ precision corpus: defensive null checks in callees and test assertions",
        ),
    },
    Rule {
        id: "ctuuninitvar",
        class: Defect::Uninitialised,
        reason: "an uninitialised value read, across functions",
        off: None,
    },
    Rule {
        id: "danglingLifetime",
        class: Defect::UseAfterFree,
        reason: "a non-local variable left pointing at an object that dies",
        off: Some(
            "precision 0/1 on the C/C++ precision corpus: a pointer to a heap object owned by a moved unique_ptr",
        ),
    },
    Rule {
        id: "danglingReference",
        class: Defect::UseAfterFree,
        reason: "a non-local reference bound to a local variable",
        off: None,
    },
    Rule {
        id: "danglingTempReference",
        class: Defect::UseAfterFree,
        reason: "a reference to a temporary used after the temporary died",
        off: None,
    },
    Rule {
        id: "danglingTemporaryLifetime",
        class: Defect::UseAfterFree,
        reason: "an object that points into a temporary used after the temporary died",
        off: None,
    },
    Rule {
        id: "deallocret",
        class: Defect::UseAfterFree,
        reason: "a pointer returned or dereferenced after it was released",
        off: None,
    },
    Rule {
        id: "deallocuse",
        class: Defect::UseAfterFree,
        reason: "a pointer dereferenced after it was released",
        off: None,
    },
    Rule {
        id: "derefInvalidIterator",
        class: Defect::UseAfterFree,
        reason: "an iterator dereferenced when it can be invalid (end)",
        off: None,
    },
    Rule {
        id: "doubleFree",
        class: Defect::UseAfterFree,
        reason: "memory released twice",
        off: None,
    },
    Rule {
        id: "eraseDereference",
        class: Defect::UseAfterFree,
        reason: "an iterator used after erase invalidated it",
        off: None,
    },
    Rule {
        id: "eraseIteratorOutOfBounds",
        class: Defect::OutOfBounds,
        reason: "erase called on an iterator past the container's end",
        off: None,
    },
    Rule {
        id: "eraseIteratorOutOfBoundsCond",
        class: Defect::OutOfBounds,
        reason: "erase called on an iterator past the end when a condition holds",
        off: None,
    },
    Rule {
        id: "floatConversionOverflow",
        class: Defect::Overflow,
        reason: "a floating-point value converted to an integer type it does not fit",
        off: None,
    },
    Rule {
        id: "identicalConditionAfterEarlyExit",
        class: Defect::WrongOperator,
        reason: "the same condition tested again after an early exit: the second test is always false",
        off: None,
    },
    Rule {
        id: "identicalInnerCondition",
        class: Defect::WrongOperator,
        reason: "an inner if with the outer if's condition: always true",
        off: Some(
            "precision 0/1 on the C/C++ precision corpus: true only in the one (no-define) configuration checked",
        ),
    },
    Rule {
        id: "ignoredReturnValue",
        class: Defect::UncheckedResult,
        reason: "the result of a function whose result must be used is dropped",
        off: None,
    },
    Rule {
        id: "incompleteArrayFill",
        class: Defect::WrongOperator,
        reason: "memset/memcpy sized in elements, not bytes",
        off: None,
    },
    Rule {
        id: "incorrectCharBooleanError",
        class: Defect::WrongOperator,
        reason: "a char literal used as a boolean: always true",
        off: None,
    },
    Rule {
        id: "incorrectLogicOperator",
        class: Defect::WrongOperator,
        reason: "a logical expression that is always true or always false (&& for ||)",
        off: None,
    },
    Rule {
        id: "incorrectStringBooleanError",
        class: Defect::WrongOperator,
        reason: "a string literal used as a boolean: always true",
        off: None,
    },
    Rule {
        id: "incorrectStringCompare",
        class: Defect::WrongOperator,
        reason: "a substring compared with a literal of a different length",
        off: None,
    },
    Rule {
        id: "integerOverflow",
        class: Defect::Overflow,
        reason: "signed integer overflow",
        off: None,
    },
    Rule {
        id: "integerOverflowCond",
        class: Defect::Overflow,
        reason: "signed integer overflow when a condition holds",
        off: None,
    },
    Rule {
        id: "invalidContainer",
        class: Defect::UseAfterFree,
        reason: "a reference, pointer or iterator into a container used after the container reallocated",
        off: None,
    },
    Rule {
        id: "invalidContainerLoop",
        class: Defect::UseAfterFree,
        reason: "a container changed while a loop iterates over it",
        off: None,
    },
    Rule {
        id: "invalidFree",
        class: Defect::UseAfterFree,
        reason: "free of an address malloc did not return",
        off: None,
    },
    Rule {
        id: "invalidIterator1",
        class: Defect::UseAfterFree,
        reason: "an invalid iterator used",
        off: None,
    },
    Rule {
        id: "invalidLifetime",
        class: Defect::UseAfterFree,
        reason: "an object used after its scope ended",
        off: None,
    },
    Rule {
        id: "invalidScanfFormatWidth",
        class: Defect::OutOfBounds,
        reason: "a scanf field width larger than the destination buffer",
        off: None,
    },
    Rule {
        id: "invalidTestForOverflow",
        class: Defect::Overflow,
        reason: "an overflow test that relies on signed overflow (undefined, optimised away)",
        off: None,
    },
    Rule {
        id: "invalidscanf",
        class: Defect::OutOfBounds,
        reason: "scanf %s without a field width",
        off: None,
    },
    Rule {
        id: "leakNoVarFunctionCall",
        class: Defect::ResourceLeak,
        reason: "an allocation passed to a function that does not release it",
        off: None,
    },
    Rule {
        id: "leakReturnValNotUsed",
        class: Defect::ResourceLeak,
        reason: "the result of an allocation function not stored",
        off: None,
    },
    Rule {
        id: "leakUnsafeArgAlloc",
        class: Defect::ResourceLeak,
        reason: "an allocation in an argument list that leaks if another argument throws",
        off: None,
    },
    Rule {
        id: "legacyUninitvar",
        class: Defect::Uninitialised,
        reason: "a variable read before it was initialised (the older checker)",
        off: None,
    },
    Rule {
        id: "literalWithCharPtrCompare",
        class: Defect::WrongOperator,
        reason: "a string literal compared with a pointer: strcmp was meant",
        off: None,
    },
    Rule {
        id: "memleak",
        class: Defect::ResourceLeak,
        reason: "allocated memory never released",
        off: Some(
            "precision 0/1 on the C/C++ precision corpus: a block returned with an offset and freed by its matching function",
        ),
    },
    Rule {
        id: "memleakOnRealloc",
        class: Defect::ResourceLeak,
        reason: "p = realloc(p, n): the old block leaks when realloc fails",
        off: None,
    },
    Rule {
        id: "memsetValueOutOfRange",
        class: Defect::WrongOperator,
        reason: "a memset fill value that does not fit an unsigned char",
        off: None,
    },
    Rule {
        id: "memsetZeroBytes",
        class: Defect::WrongOperator,
        reason: "memset of 0 bytes: swapped arguments",
        off: None,
    },
    Rule {
        id: "mismatchAllocDealloc",
        class: Defect::UseAfterFree,
        reason: "memory released by a deallocator that does not match its allocator",
        off: None,
    },
    Rule {
        id: "missingReturn",
        class: Defect::WrongOperator,
        reason: "a path out of a non-void function without a return value",
        off: None,
    },
    Rule {
        id: "moduloAlwaysTrueFalse",
        class: Defect::WrongOperator,
        reason: "a comparison of a modulo result that is always true or false",
        off: None,
    },
    Rule {
        id: "multiplySizeof",
        class: Defect::WrongOperator,
        reason: "sizeof multiplied by sizeof",
        off: None,
    },
    Rule {
        id: "negativeArraySize",
        class: Defect::OutOfBounds,
        reason: "an array declared with a negative size",
        off: None,
    },
    Rule {
        id: "negativeContainerIndex",
        class: Defect::OutOfBounds,
        reason: "a container indexed with a negative value",
        off: None,
    },
    Rule {
        id: "negativeIndex",
        class: Defect::OutOfBounds,
        reason: "an array indexed with a negative value",
        off: None,
    },
    Rule {
        id: "negativeMemoryAllocationSize",
        class: Defect::OutOfBounds,
        reason: "an allocation of a negative size",
        off: None,
    },
    Rule {
        id: "nullPointer",
        class: Defect::NullDereference,
        reason: "a null pointer dereferenced",
        off: None,
    },
    Rule {
        id: "nullPointerArithmetic",
        class: Defect::NullDereference,
        reason: "pointer arithmetic on a null pointer",
        off: None,
    },
    Rule {
        id: "nullPointerArithmeticRedundantCheck",
        class: Defect::NullDereference,
        reason: "pointer arithmetic on a pointer an earlier check says can be null",
        off: None,
    },
    Rule {
        id: "nullPointerDefaultArg",
        class: Defect::NullDereference,
        reason: "a pointer parameter whose default is null dereferenced",
        off: None,
    },
    Rule {
        id: "nullPointerOutOfMemory",
        class: Defect::NullDereference,
        reason: "an allocation's result dereferenced without a check for failure",
        off: None,
    },
    Rule {
        id: "nullPointerOutOfResources",
        class: Defect::NullDereference,
        reason: "a resource function's result (fopen) dereferenced without a check for failure",
        off: None,
    },
    Rule {
        id: "nullPointerRedundantCheck",
        class: Defect::NullDereference,
        reason: "a pointer dereferenced where an earlier or later check says it can be null",
        off: Some(
            "precision 1/19 on the C/C++ precision corpus (19 findings, with the test-assertion libraries; 46 without, 0/20): defensive checks in helpers no caller reaches, re-checks after reassignment; without the test directories 0/17",
        ),
    },
    Rule {
        id: "objectIndex",
        class: Defect::OutOfBounds,
        reason: "the address of a single object indexed past it",
        off: None,
    },
    Rule {
        id: "oppositeInnerCondition",
        class: Defect::WrongOperator,
        reason: "an inner if whose condition contradicts the outer one: dead code",
        off: None,
    },
    Rule {
        id: "overlappingStrcmp",
        class: Defect::WrongOperator,
        reason: "two strcmp comparisons that overlap: one of them is redundant or wrong",
        off: None,
    },
    Rule {
        id: "pointerAdditionResultNotNull",
        class: Defect::WrongOperator,
        reason: "ptr + n compared with null: never null unless it overflowed",
        off: None,
    },
    Rule {
        id: "pointerOutOfBounds",
        class: Defect::OutOfBounds,
        reason: "pointer arithmetic past an object's end",
        off: None,
    },
    Rule {
        id: "pointerOutOfBoundsCond",
        class: Defect::OutOfBounds,
        reason: "pointer arithmetic past an object's end when a condition holds",
        off: None,
    },
    Rule {
        id: "pointerSize",
        class: Defect::WrongOperator,
        reason: "the size of a pointer used where the size of its data was meant",
        off: None,
    },
    Rule {
        id: "resourceLeak",
        class: Defect::ResourceLeak,
        reason: "a file or other resource never closed",
        off: None,
    },
    Rule {
        id: "returnDanglingLifetime",
        class: Defect::UseAfterFree,
        reason: "an object returned that points at a local that dies",
        off: Some(
            "precision 0/2 on the C/C++ precision corpus: it took the address of a function for a dangling local",
        ),
    },
    Rule {
        id: "returnReference",
        class: Defect::UseAfterFree,
        reason: "a reference to a local variable returned",
        off: None,
    },
    Rule {
        id: "returnTempReference",
        class: Defect::UseAfterFree,
        reason: "a reference to a temporary returned",
        off: None,
    },
    Rule {
        id: "selfInitialization",
        class: Defect::Uninitialised,
        reason: "a member initialised with itself",
        off: Some(
            "precision 0/6 on the C/C++ precision corpus: it reads m(m) with a constructor parameter of the same name, which is correct C++",
        ),
    },
    Rule {
        id: "shiftNegative",
        class: Defect::Overflow,
        reason: "a shift by a negative amount",
        off: None,
    },
    Rule {
        id: "shiftNegativeLHS",
        class: Defect::Overflow,
        reason: "a negative value shifted (undefined before C++20)",
        off: None,
    },
    Rule {
        id: "shiftTooManyBits",
        class: Defect::Overflow,
        reason: "a shift by at least the operand's width",
        off: None,
    },
    Rule {
        id: "shiftTooManyBitsSigned",
        class: Defect::Overflow,
        reason: "a signed value shifted into or past its sign bit",
        off: None,
    },
    Rule {
        id: "signConversion",
        class: Defect::Overflow,
        reason: "a value that can be negative converted to an unsigned type",
        off: None,
    },
    Rule {
        id: "signConversionCond",
        class: Defect::Overflow,
        reason: "a value that can be negative when a condition holds converted to an unsigned type",
        off: None,
    },
    Rule {
        id: "signedCharArrayIndex",
        class: Defect::OutOfBounds,
        reason: "a signed char used as an array index: a negative index",
        off: None,
    },
    Rule {
        id: "sizeofCalculation",
        class: Defect::WrongOperator,
        reason: "a calculation inside sizeof, which is never evaluated",
        off: None,
    },
    Rule {
        id: "sizeofDivisionMemfunc",
        class: Defect::WrongOperator,
        reason: "a memset/memcpy size divided by sizeof where multiplying was meant",
        off: None,
    },
    Rule {
        id: "sizeofwithsilentarraypointer",
        class: Defect::WrongOperator,
        reason: "sizeof of an array parameter: the size of a pointer",
        off: None,
    },
    Rule {
        id: "stlOutOfBounds",
        class: Defect::OffByOne,
        reason: "a container indexed with a value that can equal its size",
        off: None,
    },
    Rule {
        id: "stlcstr",
        class: Defect::UseAfterFree,
        reason: "the result of c_str() used after the string died",
        off: None,
    },
    Rule {
        id: "stringCompare",
        class: Defect::WrongOperator,
        reason: "a string compared with itself",
        off: None,
    },
    Rule {
        id: "suspiciousCase",
        class: Defect::WrongOperator,
        reason: "a case label with || or &&: matches one value only",
        off: None,
    },
    Rule {
        id: "suspiciousSemicolon",
        class: Defect::WrongOperator,
        reason: "a ; right after if, for or while: the body is empty",
        off: None,
    },
    Rule {
        id: "thisUseAfterFree",
        class: Defect::UseAfterFree,
        reason: "a member used after this may have been deleted",
        off: None,
    },
    Rule {
        id: "uninitDerivedMemberVar",
        class: Defect::Uninitialised,
        reason: "a member a derived class's constructor leaves uninitialised",
        off: None,
    },
    Rule {
        id: "uninitDerivedMemberVarPrivate",
        class: Defect::Uninitialised,
        reason: "a private member a derived class's constructor leaves uninitialised",
        off: None,
    },
    Rule {
        id: "uninitMemberVar",
        class: Defect::Uninitialised,
        reason: "a member the constructor leaves uninitialised",
        off: Some(
            "precision 1/18 on the C/C++ precision corpus (18 findings, with the test-assertion libraries): storage buffers written before they are read, members set by an init method, intentionally trivial constructors",
        ),
    },
    Rule {
        id: "uninitMemberVarPrivate",
        class: Defect::Uninitialised,
        reason: "a private member the constructor leaves uninitialised",
        off: None,
    },
    Rule {
        id: "uninitStructMember",
        class: Defect::Uninitialised,
        reason: "a struct member read before it was initialised",
        off: None,
    },
    Rule {
        id: "uninitdata",
        class: Defect::Uninitialised,
        reason: "allocated memory read before it was initialised",
        off: None,
    },
    Rule {
        id: "uninitvar",
        class: Defect::Uninitialised,
        reason: "a variable read before it was initialised",
        off: Some(
            "precision 0/2 on the C/C++ precision corpus: reads guarded by a short-circuit or a zero length",
        ),
    },
    Rule {
        id: "unknownEvaluationOrder",
        class: Defect::WrongOperator,
        reason: "an expression whose result depends on unspecified evaluation order",
        off: None,
    },
    Rule {
        id: "useClosedFile",
        class: Defect::UseAfterFree,
        reason: "a file used after it was closed",
        off: None,
    },
    Rule {
        id: "uselessAssignmentPtrArg",
        class: Defect::WrongOperator,
        reason: "an assignment to a pointer parameter that has no effect: a missing dereference",
        off: None,
    },
    Rule {
        id: "uselessCallsRemove",
        class: Defect::UncheckedResult,
        reason: "the result of std::remove ignored: the elements stay in the container",
        off: None,
    },
    Rule {
        id: "va_list_usedBeforeStarted",
        class: Defect::Uninitialised,
        reason: "a va_list used before va_start",
        off: None,
    },
    Rule {
        id: "zerodiv",
        class: Defect::DivisionByZero,
        reason: "a division by zero",
        off: None,
    },
    Rule {
        id: "zerodivcond",
        class: Defect::DivisionByZero,
        reason: "a division by a value an earlier or later check says can be zero",
        off: None,
    },
];
