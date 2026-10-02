Model.where('table.column >= ?', value)
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where(table: { column: value.. })` instead of manually constructing SQL.
