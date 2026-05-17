# Rexgen

Rexgen is a command-line context for analyzing and emitting the finite UTF-8 string corpus described by a regular expression. The language below keeps product terms distinct from implementation details.

## Language

**Pattern**:
A Rust regex expression supplied by the user as the subject of analysis or generation.
_Avoid_: Regex input, expression, query

**Match String**:
A UTF-8 string accepted by a **Pattern**.
_Avoid_: Permutation, password, word

**Corpus**:
The full set of **Match Strings** described by a **Pattern** after length constraints are applied.
_Avoid_: Wordlist, permutations, output set

**Corpus Module**:
The module that owns **Pattern** parsing, **Length Constraint** validation, and **Corpus** operations.
_Avoid_: Regex service, pattern handler

**Match Count**:
The number of strings in a **Corpus**.
_Avoid_: Perm count, combinations

**Corpus Byte Size**:
The sum of UTF-8 byte lengths for every **Match String** in a **Corpus**.
_Avoid_: File size, memory size

**Byte Length**:
The UTF-8 byte length of one **Match String**.
_Avoid_: Character length, regex length

**Length Constraint**:
A minimum or maximum **Byte Length** that narrows the **Corpus**.
_Avoid_: Size constraint, char limit

**Generation Limit**:
A maximum number of **Match Strings** to emit.
_Avoid_: Count limit, cap

**Start Match String**:
A **Match String** used as the first emitted value during generation.
_Avoid_: Starting string, first word, resume point

**Stop Match String**:
A **Match String** used as the final emitted value during generation.
_Avoid_: Ending string, last word, stop point

**Total Byte Limit**:
A maximum cumulative emitted byte count used while generating strings.
_Avoid_: Size limit, max output size

**Generation Order**:
The ordering policy used while emitting **Match Strings**.
_Avoid_: Fast mode, parallel order

**Emission Transform**:
A change applied to emitted output that does not redefine the **Corpus**.
_Avoid_: Corpus transform, pattern transform

**Generation Confirmation**:
An explicit user approval step before emitting **Match Strings**.
_Avoid_: Safety prompt, confirm flag

**Completion Script**:
A shell integration script generated for command-line argument completion.
_Avoid_: Shell helper, autocomplete file

**Command Help**:
Generated CLI usage text for the top-level command or one of its subcommands.
_Avoid_: Usage guide, tutorial

## Relationships

- A **Pattern** describes zero or more **Match Strings**.
- A **Corpus** contains every **Match String** accepted by a **Pattern** after **Length Constraints** are applied.
- A **Corpus Module** parses one **Pattern** and applies one set of **Length Constraints**.
- A **Match Count** is calculated from exactly one **Corpus**.
- A **Corpus Byte Size** is calculated from exactly one **Corpus**.
- A **Length Constraint** affects both **Match Count** and generated **Match Strings**.
- A **Generation Limit** affects generated **Match Strings**, but does not redefine the **Corpus**.
- A **Start Match String** affects generated **Match Strings**, but does not redefine the **Corpus**.
- A **Stop Match String** affects generated **Match Strings**, but does not redefine the **Corpus**.
- A **Total Byte Limit** affects emitted output, but does not redefine the **Corpus Byte Size**.
- **Generation Order** affects the sequence of emitted **Match Strings**, but does not redefine the **Corpus**.
- An **Emission Transform** affects emitted output, but does not redefine the **Corpus**.
- **Generation Confirmation** applies before generating **Match Strings** unless explicitly bypassed.
- A **Completion Script** describes the CLI interface, not any **Pattern** or **Corpus**.
- **Command Help** describes available commands, arguments, and options; it does not analyze a **Pattern**.

## Example Dialogue

> **Dev:** "For pattern `[ab]{1,3}`, should `--limit 2` change the match count?"
> **Domain expert:** "No. The **Generation Limit** only stops emission. The **Match Count** still belongs to the full **Corpus** after **Length Constraints**."

## Flagged Ambiguities

- "size" can mean one string's **Byte Length**, total **Corpus Byte Size**, or emitted output bounded by a **Total Byte Limit**; use the precise term in docs and UI text.
- "strings that the regex can match" is resolved as **Match Strings**, and only valid UTF-8 strings are in scope.
- "permutation" is avoided because regex alternation and repetition define a **Corpus**, not necessarily mathematical permutations.
