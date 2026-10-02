execute(<<~SQL.squish, "Post Load")
  SELECT * FROM posts
    WHERE post_id = 1
SQL
