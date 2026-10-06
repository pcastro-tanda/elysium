where('name != ?', 'Gabe')
^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where.not(name: 'Gabe')` instead of manually constructing negated SQL in `where`.
