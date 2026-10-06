Model.where('column >= :min AND column < :max', min: value1, max: value2)
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where(column: value1...value2)` instead of manually constructing SQL.
