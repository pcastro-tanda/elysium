# Style/MapIntoArray

Checks for usages of `each` with `<<`, `push`, or `append` which can be replaced by `map`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

If `PreferredMethods` is configured for `map` in `Style/CollectionMethods`, this cop uses the specified method for replacement.

NOTE: The return value of `Enumerable#each` is `self`, whereas the return value of `Enumerable#map` is an `Array`. They are not autocorrected when a return value could be used because these types differ.

NOTE: It only detects when the mapping destination is either: a local variable initialized as an empty array and referred to only by the pushing operation; or, if it is the single block argument to a `[].tap` block. This is because, if not, it's challenging to statically guarantee that the mapping destination variable remains an empty array.

@safety
  This cop is unsafe because not all objects that have an `each` method also have a `map` method (e.g. `ENV`). Additionally, for calls with a block, not all objects that have a `map` method return an array (e.g. `Enumerator::Lazy`).

## Options

This rule has no options.

## Blind spots

None recorded.
