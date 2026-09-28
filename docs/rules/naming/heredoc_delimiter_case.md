# Naming/HeredocDelimiterCase

Checks that your heredocs are using the configured case.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

By default it is configured to enforce uppercase heredocs.

```ruby
# EnforcedStyle: uppercase (default)
# bad
<<-sql
  SELECT * FROM foo
sql

# good
<<-SQL
  SELECT * FROM foo
SQL
```

```ruby
# EnforcedStyle: lowercase
# bad
<<-SQL
  SELECT * FROM foo
SQL

# good
<<-sql
  SELECT * FROM foo
sql
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `uppercase` | `uppercase`, `lowercase` | Whether heredoc delimiters should be uppercase or lowercase. |

## Blind spots

None recorded.
