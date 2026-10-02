User.where("name IN (:names)", names: ['john', 'jane'])
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where(name: ['john', 'jane'])` instead of manually constructing SQL.
