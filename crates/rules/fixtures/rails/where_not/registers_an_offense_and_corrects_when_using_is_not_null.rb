User.where('name IS NOT NULL')
     ^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where.not(name: nil)` instead of manually constructing negated SQL in `where`.
