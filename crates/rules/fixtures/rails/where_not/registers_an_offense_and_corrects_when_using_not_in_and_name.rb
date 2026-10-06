User.where("name NOT IN (:names)", names: ['john', 'jane'])
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where.not(name: ['john', 'jane'])` instead of manually constructing negated SQL in `where`.
