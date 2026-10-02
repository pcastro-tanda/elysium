Model.where.not('column >= ?', value)
            ^^^^^^^^^^^^^^^^^^^^^^^^^ Use `not(column: value..)` instead of manually constructing SQL.
