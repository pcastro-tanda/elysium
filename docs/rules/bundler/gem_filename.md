# Bundler/GemFilename

Verifies that a project contains Gemfile or gems.rb file and correct associated lock file based on the configuration.

| | |
| --- | --- |
| Department | Bundler |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Verifies that a project contains Gemfile or gems.rb file and correct
associated lock file based on the configuration.

```ruby
# EnforcedStyle: Gemfile (default)

# bad
Project contains gems.rb and gems.locked files

# bad
Project contains Gemfile and gems.locked file

# good
Project contains Gemfile and Gemfile.lock
```

```ruby
# EnforcedStyle: gems.rb

# bad
Project contains Gemfile and Gemfile.lock files

# bad
Project contains gems.rb and Gemfile.lock file

# good
Project contains gems.rb and gems.locked files
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `Gemfile` | `Gemfile`, `gems.rb` | Which gem file naming convention to enforce. |

## Blind spots

Only the currently linted file's own basename is checked, matching
upstream's `on_new_investigation`; this cop never inspects the filesystem
for the *other* file of the pair, so linting `Gemfile.lock` in isolation
(with the `gems.rb` style) still reports a mismatch even if a `gems.rb` was
in fact never created, and vice versa -- exactly upstream's behavior, since
it too checks only the one file it happens to be linting.
