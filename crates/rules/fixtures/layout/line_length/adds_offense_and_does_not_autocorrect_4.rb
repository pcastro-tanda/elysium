foo = <<-SQL
  SELECT a b c d a b FROM c d a b c d ; COUNT(*) a b
                                        ^^^^^^^^^^^^ Line is too long. [52/40]
SQL
