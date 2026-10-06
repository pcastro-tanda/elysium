# Rails/SquishedSQLHeredocs

Checks SQL heredocs to use `.squish`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks SQL heredocs to use `.squish`.

Some SQL syntax (e.g. PostgreSQL comments and functions) requires newlines to be preserved in order to work, thus autocorrection for this cop is not safe.

```ruby
# bad
<<-SQL
  SELECT * FROM posts;
SQL

<<-SQL
  SELECT * FROM posts
    WHERE id = 1
SQL

execute(<<~SQL, "Post Load")
  SELECT * FROM posts
    WHERE post_id = 1
SQL

# good
<<-SQL.squish
  SELECT * FROM posts;
SQL

<<~SQL.squish
  SELECT * FROM table
    WHERE id = 1
SQL

execute(<<~SQL.squish, "Post Load")
  SELECT * FROM posts
    WHERE post_id = 1
SQL
```

## Options

This rule has no options.

## Blind spots

None recorded.
