Model.where(['column >= ? AND column <= ?', value1, value2])
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where(column: value1..value2)` instead of manually constructing SQL.
