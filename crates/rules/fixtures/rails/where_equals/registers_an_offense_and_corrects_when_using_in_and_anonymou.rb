User.where("name IN (?)", ['john', 'jane'])
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where(name: ['john', 'jane'])` instead of manually constructing SQL.
