Model.where('  column  >= ?   ', value)
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where(column: value..)` instead of manually constructing SQL.
Model.where(' column >=     :min    ', min: value)
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where(column: value..)` instead of manually constructing SQL.
Model.where(' column   >=    ?   AND   column  <   ? ', value1, value2)
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where(column: value1...value2)` instead of manually constructing SQL.
Model.where(' column   >=  :min  AND column   <   :max    ', min: value1, max: value2)
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where(column: value1...value2)` instead of manually constructing SQL.
