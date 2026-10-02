execute(<<~SQL, "Post Load")
        ^^^^^^ Use `<<~SQL.squish` instead of `<<~SQL`.
  SELECT * FROM posts
    WHERE post_id = 1
SQL
