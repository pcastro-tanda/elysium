execute(<<-SQL, "Post Load")
  SELECT * FROM posts
  -- This is a comment, so squish can't be used
    WHERE post_id = 1
SQL
