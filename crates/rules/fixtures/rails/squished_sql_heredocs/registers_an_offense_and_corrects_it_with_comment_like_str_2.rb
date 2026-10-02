<<~SQL
^^^^^^ Use `<<~SQL.squish` instead of `<<~SQL`.
  SELECT * FROM posts WHERE [--another identifier!] = 1 AND "-- this is a name, not a comment" = '-- this is a string, not a comment';
SQL
