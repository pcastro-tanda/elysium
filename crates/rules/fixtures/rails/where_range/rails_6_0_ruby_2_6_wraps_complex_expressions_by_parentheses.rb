Model.where('column >= ?', true ? 1 : 2)
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where(column: (true ? 1 : 2)..)` instead of manually constructing SQL.
