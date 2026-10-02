# Style/RedundantSelfAssignmentBranch

Checks for places where conditional branch makes redundant self-assignment.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

It only detects local variable because it may replace state of instance variable, class variable, and global variable that have state across methods with `nil`.

## Options

This rule has no options.

## Blind spots

None recorded.
