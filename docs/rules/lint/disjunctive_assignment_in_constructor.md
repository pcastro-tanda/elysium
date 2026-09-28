# Lint/DisjunctiveAssignmentInConstructor

In constructor, plain assignment is preferred over disjunctive.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks constructors for disjunctive assignments (`||=`) that should be plain assignments.

So far, this cop is only concerned with disjunctive assignment of instance variables.

In ruby, an instance variable is nil until a value is assigned, so the disjunction is unnecessary. A plain assignment has the same effect.

## Options

This rule has no options.

## Blind spots

None recorded.
