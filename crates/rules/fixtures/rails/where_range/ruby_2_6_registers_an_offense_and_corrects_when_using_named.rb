Model.where('column >= :min', min: value)
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where(column: value..)` instead of manually constructing SQL.
